//! Live process execution for ` ```tty ` blocks.
//!
//! Each block runs a shell command in a real pseudo-terminal ([`portable_pty`]),
//! so programs see a TTY and emit colors / full-screen redraws as usual. A
//! per-process reader thread feeds the raw bytes into a [`vt100`] parser that
//! maintains a virtual screen grid; the present loop paints that grid into a
//! bordered box (see [`super`]).
//!
//! [`ProcessManager`] owns the running processes and implements the cross-slide
//! lifecycle:
//!
//! - **Ephemeral** processes (no `id`, or no `persist`) are keyed by their
//!   `(slide, block)` position and killed when that slide stops being current.
//! - **Persistent** processes (`persist` *and* an `id`) are keyed by `id` and
//!   survive a slide change only when the destination slide reuses that `id` —
//!   then the same process (and its accumulated output) is re-mounted.
//!
//! A [`ManagedProc`] kills its child on [`Drop`], so dropping it from either map
//! (via [`ProcessManager::reconcile`]) or dropping the whole manager tears the
//! process down.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use super::parse::{CodeBlock, Segment, TtyBlock};

/// Default box interior when a block sets no `rows=`/`cols=`.
const DEFAULT_ROWS: u16 = 10;
const DEFAULT_COLS: u16 = 80;
/// Lines of scrollback the emulator keeps; the box shows the bottom of it.
const SCROLLBACK: usize = 1000;

/// The Neovim invocation for a code block: read-only, no swap/shada, and a
/// single `User VeryLazy` autocmd (fired after the user's plugins finish
/// loading) that strips *all* UI chrome — statusline, tabline, command line,
/// rulers, sign/fold/number/status columns, list/wrap, and end-of-buffer fill.
/// Nothing in the user's own config is modified; their colorscheme and
/// Treesitter highlighting still apply.
const NVIM_CHROME_OFF: &str = "autocmd User VeryLazy ++once set laststatus=0 \
showtabline=0 cmdheight=0 noruler noshowmode signcolumn=no foldcolumn=0 \
statuscolumn= nonumber norelativenumber nocursorline nocursorcolumn nolist \
nowrap fillchars=eob:\\  | redraw!";

/// Keystrokes sent to a started editor (over the PTY, never editing config):
/// `\r` dismisses any blocking "hit-enter" prompt, and the `:lua …` command
/// closes any floating window — LSP progress, diagnostics, or notification
/// popups that the user's full config flashes over the code. Sent a few times
/// across the editor's async startup so late popups are caught too.
const NVIM_CLEAR_POPUPS: &[u8] = b"\r:lua for _,w in ipairs(vim.api.nvim_list_wins()) do \
local c=vim.api.nvim_win_get_config(w) if c.relative~=\"\" then \
pcall(vim.api.nvim_win_close,w,true) end end\r";

/// Names temp files for code blocks uniquely within the process.
static TEMP_SEQ: AtomicU32 = AtomicU32::new(0);

/// What to run in a managed pseudo-terminal.
#[derive(Debug)]
enum Spawn<'a> {
    /// A `tty` block: run `$SHELL -lc <command>`.
    Shell(&'a str),
    /// A code block: open the code in Neovim with all chrome removed.
    Editor { lang: &'a str, code: &'a str },
}

/// A single running (or finished) `tty` process and its emulator screen.
pub(crate) struct ManagedProc {
    /// The vt100 emulator, fed by the reader thread; locked to read the screen.
    parser: Arc<Mutex<vt100::Parser>>,
    /// The PTY master, kept for resizing as the box size changes.
    master: Box<dyn MasterPty + Send>,
    /// The child, kept for status polling and for `kill` on drop.
    child: Box<dyn Child + Send + Sync>,
    started: Instant,
    /// When the child exited (frozen elapsed time), once observed.
    ended: Option<Instant>,
    /// The exit status, once the child has finished.
    status: Option<portable_pty::ExitStatus>,
    rows: u16,
    cols: u16,
    /// A temp file backing a code block, deleted on drop.
    temp: Option<PathBuf>,
    /// The reader thread; detached — it ends on EOF when the child dies.
    _reader: JoinHandle<()>,
}

impl ManagedProc {
    /// Spawn `what` in a `rows`x`cols` PTY and start streaming its output into a
    /// fresh emulator screen.
    fn spawn(what: &Spawn<'_>, rows: u16, cols: u16) -> Result<Self> {
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = native_pty_system()
            .openpty(size)
            .context("opening a pseudo-terminal")?;

        let (mut cmd, temp) = build_command(what)?;
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }
        // Make sure programs assume a capable, colorful terminal.
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let child = pair
            .slave
            .spawn_command(cmd)
            .context("spawning a pseudo-terminal command")?;
        // Drop the slave so the master sees EOF once the child exits.
        drop(pair.slave);

        // An editor can emit startup messages (e.g. nvim's DSR-probe warning, a
        // plugin notification) that, with the command line hidden, force a
        // blocking "hit-enter" prompt. Send a couple of dismiss keys over the
        // PTY, then `gg` to return the cursor to the top. This touches no config.
        if matches!(what, Spawn::Editor { .. })
            && let Ok(mut writer) = pair.master.take_writer()
        {
            std::thread::spawn(move || {
                // Cumulative delays span the editor's async startup and the
                // LSP-progress window, so both early and late popups are cleared.
                for delay in [250_u64, 300, 350, 450, 600, 600] {
                    std::thread::sleep(Duration::from_millis(delay));
                    let _ = writer.write_all(NVIM_CLEAR_POPUPS);
                    let _ = writer.flush();
                }
                std::thread::sleep(Duration::from_millis(200));
                let _ = writer.write_all(b"gg"); // reset cursor/view to the top
                let _ = writer.flush();
            });
        }

        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, SCROLLBACK)));
        let mut reader = pair
            .master
            .try_clone_reader()
            .context("cloning the tty reader")?;
        let sink = Arc::clone(&parser);
        let reader_thread = std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        if let Ok(mut emulator) = sink.lock() {
                            emulator.process(&buf[..read]);
                        }
                    }
                }
            }
        });

        Ok(Self {
            parser,
            master: pair.master,
            child,
            started: Instant::now(),
            ended: None,
            status: None,
            rows,
            cols,
            temp,
            _reader: reader_thread,
        })
    }

    /// Lock the emulator so the caller can read its screen for rendering.
    pub(crate) fn lock_parser(&self) -> MutexGuard<'_, vt100::Parser> {
        // A panicked reader thread is the only poisoning source; recover the
        // grid anyway so a dead block still renders its last frame.
        self.parser
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Note the child's exit (once), freezing the elapsed time and status.
    fn poll(&mut self) {
        if self.status.is_some() {
            return;
        }
        if let Ok(Some(status)) = self.child.try_wait() {
            self.status = Some(status);
            self.ended = Some(Instant::now());
        }
    }

    /// Resize the PTY and emulator to a new box interior, if it changed.
    fn resize(&mut self, rows: u16, cols: u16) {
        if rows == self.rows && cols == self.cols {
            return;
        }
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        self.lock_parser().screen_mut().set_size(rows, cols);
        self.rows = rows;
        self.cols = cols;
    }

    /// Whether the child is still running.
    pub(crate) fn is_running(&self) -> bool {
        self.status.is_none()
    }

    /// The current box interior size `(rows, cols)` the PTY/emulator is set to.
    pub(crate) fn interior(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    /// Wall-clock time since spawn (frozen at exit).
    fn elapsed(&self) -> Duration {
        self.ended.unwrap_or_else(Instant::now) - self.started
    }

    /// The default status-bar text: a (blink-toggled) running indicator with
    /// elapsed time, or the exit mark and code once finished.
    pub(crate) fn status_line(&self, blink_on: bool) -> String {
        let secs = self.elapsed().as_secs();
        let clock = format!("{:02}:{:02}", secs / 60, secs % 60);
        match &self.status {
            None => {
                let dot = if blink_on { "●" } else { " " };
                format!(" {dot} running · {clock} ")
            }
            Some(status) => {
                let (mark, code) = if status.success() {
                    ("✓", status.exit_code())
                } else {
                    ("✗", status.exit_code())
                };
                format!(" {mark} exited {code} · {clock} ")
            }
        }
    }
}

impl std::fmt::Debug for ManagedProc {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The PTY master and vt100 parser are not `Debug`; summarise instead.
        formatter
            .debug_struct("ManagedProc")
            .field("rows", &self.rows)
            .field("cols", &self.cols)
            .field("running", &self.is_running())
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl Drop for ManagedProc {
    fn drop(&mut self) {
        // Best-effort teardown: kill the child and reap it so leaving a slide
        // (or quitting) never leaves a stray process. The reader thread then
        // hits EOF and exits on its own.
        let _ = self.child.kill();
        let _ = self.child.try_wait();
        if let Some(path) = &self.temp {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Build the [`CommandBuilder`] for a [`Spawn`], plus any temp file that backs
/// it (a code block writes its source to a temp file for the editor to open).
fn build_command(what: &Spawn<'_>) -> Result<(CommandBuilder, Option<PathBuf>)> {
    match what {
        Spawn::Shell(command) => {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
            let mut cmd = CommandBuilder::new(shell);
            cmd.arg("-lc");
            cmd.arg(command);
            Ok((cmd, None))
        }
        Spawn::Editor { lang, code } => {
            let path = write_code_temp(lang, code).context("writing a code block temp file")?;
            let mut cmd = CommandBuilder::new("nvim");
            for arg in ["-R", "-n", "-i", "NONE", "-c", NVIM_CHROME_OFF] {
                cmd.arg(arg);
            }
            cmd.arg(&path);
            Ok((cmd, Some(path)))
        }
    }
}

/// Write a code block to a uniquely-named temp file whose extension matches the
/// language, so the editor detects the filetype and highlights it.
fn write_code_temp(lang: &str, code: &str) -> Result<PathBuf> {
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "ahr-{}-{seq}.{}",
        std::process::id(),
        ext_for_lang(lang)
    ));
    std::fs::write(&path, code)?;
    Ok(path)
}

/// A file extension for a fence language, so the editor's filetype detection
/// (and thus syntax/Treesitter highlighting) kicks in. Unknown languages fall
/// back to the language itself as the extension (letting the editor try), or
/// `txt` when that is not usable.
fn ext_for_lang(lang: &str) -> String {
    let lower = lang.to_ascii_lowercase();
    let known = match lower.as_str() {
        "rust" => "rs",
        "js" | "javascript" => "js",
        "ts" | "typescript" => "ts",
        "jsx" => "jsx",
        "tsx" => "tsx",
        "md" | "markdown" => "md",
        "py" | "python" => "py",
        "sh" | "bash" | "shell" | "zsh" => "sh",
        "cpp" | "c++" => "cpp",
        "yaml" | "yml" => "yaml",
        "go" | "golang" => "go",
        "html" => "html",
        "css" => "css",
        "json" => "json",
        "toml" => "toml",
        "c" => "c",
        "lua" => "lua",
        "vim" => "vim",
        _ => "",
    };
    if !known.is_empty() {
        return known.to_owned();
    }
    let sanitized: String = lower.chars().filter(char::is_ascii_alphanumeric).collect();
    if sanitized.is_empty() {
        "txt".to_owned()
    } else {
        sanitized
    }
}

/// Owns every running process and applies the per-slide lifecycle.
#[derive(Debug, Default)]
pub(crate) struct ProcessManager {
    /// Ephemeral `tty` processes, keyed by `(slide index, block index)`.
    ephemeral: HashMap<(usize, usize), ManagedProc>,
    /// Persistent `tty` processes, keyed by their `id`.
    persistent: HashMap<String, ManagedProc>,
    /// Code-block editors, keyed by `(slide, block)`. Cached for the whole
    /// session (the highlighted output is static, so re-rendering on every
    /// visit would only re-show the editor's startup). `None` records a spawn
    /// that failed (e.g. no `nvim`), so it is not retried every tick.
    code: HashMap<(usize, usize), Option<ManagedProc>>,
}

impl ProcessManager {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Apply the lifecycle for navigating to slide `current`: kill ephemeral
    /// processes from other slides and persistent processes whose `id` the new
    /// slide does not reuse. Survivors are re-mounted by [`Self::ensure`].
    ///
    /// Dropping a [`ManagedProc`] kills its child, so `retain` does the killing.
    pub(crate) fn reconcile(&mut self, current: usize, segments: &[Segment]) {
        let keep = keep_persistent_ids(segments);
        self.persistent.retain(|id, _| keep.contains(id));
        self.ephemeral.retain(|(slide, _), _| *slide == current);
    }

    /// Ensure every `tty`/code block on slide `current` has a process, sized to
    /// fit the `content_rows`x`content_cols` content area, spawning the missing
    /// ones and resizing/polling the rest. `tty` boxes reserve two cells for
    /// their border; code blocks are borderless and use the full area.
    pub(crate) fn ensure(
        &mut self,
        current: usize,
        segments: &[Segment],
        content_rows: u16,
        content_cols: u16,
    ) {
        for (block_index, segment) in segments.iter().enumerate() {
            match segment {
                Segment::Markdown(_) | Segment::Mermaid(_) => {}
                Segment::Tty(block) => {
                    let (rows, cols) = box_interior(
                        block,
                        content_rows.saturating_sub(2),
                        content_cols.saturating_sub(2),
                    );
                    let spawn = Spawn::Shell(&block.command);
                    if let Some(id) = persistent_id(block) {
                        if let Some(proc) = self.persistent.get_mut(id) {
                            proc.resize(rows, cols);
                            proc.poll();
                        } else if let Ok(mut proc) = ManagedProc::spawn(&spawn, rows, cols) {
                            proc.poll();
                            self.persistent.insert(id.to_owned(), proc);
                        }
                    } else {
                        let key = (current, block_index);
                        if let Some(proc) = self.ephemeral.get_mut(&key) {
                            proc.resize(rows, cols);
                            proc.poll();
                        } else if let Ok(mut proc) = ManagedProc::spawn(&spawn, rows, cols) {
                            proc.poll();
                            self.ephemeral.insert(key, proc);
                        }
                    }
                }
                Segment::Code(block) => {
                    self.ensure_code(current, block_index, block, content_rows, content_cols);
                }
            }
        }
    }

    /// Ensure the editor for one code block is spawned and sized, caching it by
    /// `(slide, block_index)`. Used both for the current slide (via [`Self::ensure`])
    /// and to eagerly pre-spawn `render=eager` blocks at load. A failed spawn is
    /// recorded as `None` so it is not retried.
    pub(crate) fn ensure_code(
        &mut self,
        slide: usize,
        block_index: usize,
        block: &CodeBlock,
        content_rows: u16,
        content_cols: u16,
    ) {
        let (rows, cols) = code_interior(&block.code, content_rows, content_cols);
        match self.code.get_mut(&(slide, block_index)) {
            Some(Some(proc)) => {
                proc.resize(rows, cols);
                proc.poll();
            }
            Some(None) => {} // spawn failed before; don't retry
            None => {
                let spawn = Spawn::Editor {
                    lang: &block.lang,
                    code: &block.code,
                };
                let proc = ManagedProc::spawn(&spawn, rows, cols).ok();
                self.code.insert((slide, block_index), proc);
            }
        }
    }

    /// The process backing a `tty` block on slide `current`, if it spawned.
    pub(crate) fn proc_for(
        &self,
        current: usize,
        block_index: usize,
        block: &TtyBlock,
    ) -> Option<&ManagedProc> {
        match persistent_id(block) {
            Some(id) => self.persistent.get(id),
            None => self.ephemeral.get(&(current, block_index)),
        }
    }

    /// The editor backing a code block on slide `current`, if it spawned.
    pub(crate) fn code_proc(&self, current: usize, block_index: usize) -> Option<&ManagedProc> {
        self.code.get(&(current, block_index)).and_then(Option::as_ref)
    }

    /// Whether any process on slide `current` is still running (so the loop
    /// keeps repainting for live output and the blinking status indicator).
    pub(crate) fn any_running(&self, current: usize, segments: &[Segment]) -> bool {
        segments.iter().enumerate().any(|(block_index, segment)| match segment {
            Segment::Markdown(_) | Segment::Mermaid(_) => false,
            Segment::Tty(block) => self
                .proc_for(current, block_index, block)
                .is_some_and(ManagedProc::is_running),
            Segment::Code(_) => self
                .code_proc(current, block_index)
                .is_some_and(ManagedProc::is_running),
        })
    }
}

/// The box interior for a block: its `rows=`/`cols=` (or defaults), each clamped
/// to the space available and at least 1.
pub(crate) fn box_interior(block: &TtyBlock, max_rows: u16, max_cols: u16) -> (u16, u16) {
    let rows = block
        .rows
        .unwrap_or(DEFAULT_ROWS)
        .min(max_rows.max(1))
        .max(1);
    let cols = block
        .cols
        .unwrap_or(DEFAULT_COLS)
        .min(max_cols.max(1))
        .max(1);
    (rows, cols)
}

/// The interior `(rows, cols)` for a code block: exactly the code's line count
/// and widest line (so the borderless editor has no empty margin), each clamped
/// to the content area and at least 1.
pub(crate) fn code_interior(code: &str, max_rows: u16, max_cols: u16) -> (u16, u16) {
    let mut rows: u16 = 0;
    let mut cols: u16 = 0;
    for line in code.lines() {
        rows = rows.saturating_add(1);
        cols = cols.max(u16::try_from(console::measure_text_width(line)).unwrap_or(u16::MAX));
    }
    (
        rows.max(1).min(max_rows.max(1)),
        cols.max(1).min(max_cols.max(1)),
    )
}

/// The persistence key for a block: `Some(id)` only when it both `persist`s and
/// carries an `id` (the link required to survive a slide change).
fn persistent_id(block: &TtyBlock) -> Option<&str> {
    if block.persist {
        block.id.as_deref()
    } else {
        None
    }
}

/// The set of persistent `id`s a slide's segments reference — the processes
/// allowed to survive when navigating *to* that slide.
fn keep_persistent_ids(segments: &[Segment]) -> HashSet<String> {
    segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Tty(block) => persistent_id(block).map(str::to_owned),
            Segment::Markdown(_) | Segment::Code(_) | Segment::Mermaid(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{box_interior, keep_persistent_ids, persistent_id};
    use crate::present::parse::{Segment, TtyBlock};

    fn block(id: Option<&str>, persist: bool) -> TtyBlock {
        TtyBlock {
            id: id.map(str::to_owned),
            persist,
            ..TtyBlock::default()
        }
    }

    #[test]
    fn persistent_only_when_persist_and_id_present() {
        assert_eq!(persistent_id(&block(Some("a"), true)), Some("a"));
        assert_eq!(persistent_id(&block(Some("a"), false)), None);
        assert_eq!(persistent_id(&block(None, true)), None);
    }

    #[test]
    fn keep_set_collects_persistent_ids_only() {
        let segments = vec![
            Segment::Markdown("# hi".into()),
            Segment::Tty(block(Some("counter"), true)),
            Segment::Tty(block(Some("scratch"), false)), // not persistent
            Segment::Tty(block(None, true)),             // no id to link
        ];
        let keep = keep_persistent_ids(&segments);
        assert_eq!(keep.len(), 1);
        assert!(keep.contains("counter"));
    }

    #[test]
    fn box_interior_uses_defaults_then_clamps() {
        // Default rows (10), explicit wide cols clamped to what's available.
        let wide = TtyBlock {
            cols: Some(200),
            ..TtyBlock::default()
        };
        assert_eq!(box_interior(&wide, 40, 50), (10, 50));

        // Explicit small size kept as-is.
        let small = TtyBlock {
            rows: Some(4),
            cols: Some(20),
            ..TtyBlock::default()
        };
        assert_eq!(box_interior(&small, 40, 80), (4, 20));
    }
}
