//! Live process execution for `<tty>` components.
//!
//! Each component runs a shell command in a real pseudo-terminal ([`portable_pty`]),
//! so programs see a TTY and emit colors / full-screen redraws as usual. A
//! per-process reader thread feeds the raw bytes into a [`vt100`] parser that
//! maintains a virtual screen grid; the deck runtime paints that grid into a
//! bordered box (see [`super`]).
//!
//! A component may carry `keys=` — keystrokes fed into the process shortly after it
//! starts (decoded by [`decode_keys`]), useful to drive a TUI from a slide, e.g.
//! to dismiss an editor's startup popups. The deck runtime never *reads* the
//! PTY, so `keys` is a fixed startup script, not interactivity.
//!
//! [`ProcessManager`] owns the running processes and implements the cross-slide
//! lifecycle:
//!
//! - **Ephemeral** processes (no `id`, or no `persist`) are keyed by their
//!   `(slide, component)` position and killed when that slide stops being current.
//! - **Persistent** processes (`persist` *and* an `id`) are keyed by `id` and
//!   survive a slide change only when the destination slide reuses that `id` —
//!   then the same process (and its accumulated output) is re-mounted.
//! - **Prerendered** processes (`prerender`) are spawned at deck load, keyed by
//!   their `(slide, component)` position, and kept alive for the whole session, so
//!   a slow-starting program is already drawn when its slide is reached.
//!
//! A [`ManagedProc`] kills its child on [`Drop`], so dropping it from either map
//! (via [`ProcessManager::reconcile`]) or dropping the whole manager tears the
//! process down.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};
use tui_term::widget::PseudoTerminal;

use super::{Cached, FrameCtx, Item};
use crate::deck::Segment;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TtyComponent {
    /// Links the same logical process across slides (for `persist`).
    pub(crate) id: Option<String>,
    /// Keep the process alive across a slide change when the next slide has a
    /// `<tty>` with the same `id`.
    pub(crate) persist: bool,
    /// Spawn at deck load (not first view) and keep alive for the whole session.
    /// Takes precedence over `persist`/`id`: keyed by position, not shared.
    pub(crate) prerender: bool,
    /// Box title; falls back to the command's first token.
    pub(crate) title: Option<String>,
    /// Fixed status text; falls back to the live process status.
    pub(crate) status: Option<String>,
    /// Box interior, in cells; a default is used when absent.
    pub(crate) rows: Option<u16>,
    pub(crate) cols: Option<u16>,
    /// Keystrokes fed into the process after it starts (raw, with
    /// `\r`/`\n`/`\t`/`\e`/`\\` escapes), to drive a TUI. Decoded by
    /// [`decode_keys`].
    pub(crate) keys: Option<String>,
    pub(crate) command: String,
}

impl TtyComponent {
    /// Parse a `<tty>` component's attributes. Attributes are whitespace-separated
    /// `key="value"` pairs or bare flags (e.g. `persist`); unknown keys ignored.
    pub(crate) fn parse(attrs: &str, command: String) -> Self {
        let mut component = Self {
            command,
            ..Self::default()
        };
        for (key, value) in super::tokenize_attrs(attrs) {
            match key.as_str() {
                "id" => component.id = value,
                "persist" => component.persist = value.as_deref() != Some("false"),
                "prerender" => component.prerender = value.as_deref() != Some("false"),
                "title" => component.title = value,
                "status" => component.status = value,
                "rows" => component.rows = value.and_then(|val| val.parse().ok()),
                "cols" => component.cols = value.and_then(|val| val.parse().ok()),
                "keys" => component.keys = value,
                _ => {}
            }
        }
        component
    }

    /// The box title: the `title` override, else the command's first token, else
    /// `"tty"`.
    pub(crate) fn display_title(&self) -> &str {
        if let Some(title) = &self.title {
            return title;
        }
        self.command.split_whitespace().next().unwrap_or("tty")
    }
}

impl super::Component for TtyComponent {
    // Drawn live from the `ProcessManager`; nothing to pre-render.
    fn render(&self, _content_w: usize) -> Cached {
        Cached::Live
    }

    fn lay_out(&self, ctx: &FrameCtx<'_>) -> Option<Item> {
        let proc = ctx.manager.proc_for(ctx.current, ctx.index, self);
        let (rows, cols) = proc.map_or_else(
            || {
                box_interior(
                    self,
                    ctx.content.height.saturating_sub(2),
                    u16::try_from(ctx.content_w).unwrap_or(u16::MAX).saturating_sub(2),
                )
            },
            ManagedProc::interior,
        );
        Some(Item {
            width: cols.saturating_add(2).min(ctx.content.width),
            height: rows.saturating_add(2).min(ctx.content.height),
            left_align: false,
            text: None,
        })
    }

    fn draw(&self, frame: &mut Frame<'_>, area: Rect, _item: &Item, ctx: &FrameCtx<'_>) {
        let proc = ctx.manager.proc_for(ctx.current, ctx.index, self);
        draw_tty(frame, area, self, proc, ctx.blink_on);
    }

    fn is_active(&self, ctx: &FrameCtx<'_>) -> bool {
        ctx.manager
            .proc_for(ctx.current, ctx.index, self)
            .is_some_and(ManagedProc::is_running)
    }
}

/// Default box interior when a component sets no `rows=`/`cols=`.
const DEFAULT_ROWS: u16 = 10;
const DEFAULT_COLS: u16 = 80;
/// Lines of scrollback the emulator keeps; the box shows the bottom of it.
const SCROLLBACK: usize = 1000;

/// Cumulative delays (ms) at which a component's `keys=` are (re)sent into the PTY.
/// Spanning a program's async startup means late-appearing popups are caught
/// too, not just whatever is on screen the instant the process spawns.
const KEYS_DELAYS: &[u64] = &[250, 300, 350, 450, 600, 600];

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
    /// The reader thread; detached — it ends on EOF when the child dies.
    _reader: JoinHandle<()>,
}

impl ManagedProc {
    /// Spawn `command` (via `$SHELL -lc`) in a `rows`x`cols` PTY and start
    /// streaming its output into a fresh emulator screen. When `keys` is set,
    /// those bytes are fed into the PTY across the startup window (see
    /// [`KEYS_DELAYS`]).
    fn spawn(command: &str, keys: Option<&[u8]>, rows: u16, cols: u16) -> Result<Self> {
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = native_pty_system()
            .openpty(size)
            .context("opening a pseudo-terminal")?;

        let mut cmd = shell_command(command);
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

        // Feed `keys=` into the PTY across the program's async startup, so both
        // early and late prompts/popups are driven the same way every run.
        if let Some(keys) = keys
            && !keys.is_empty()
            && let Ok(mut writer) = pair.master.take_writer()
        {
            let keys = keys.to_vec();
            std::thread::spawn(move || {
                for delay in KEYS_DELAYS {
                    std::thread::sleep(Duration::from_millis(*delay));
                    if writer.write_all(&keys).and_then(|()| writer.flush()).is_err() {
                        break;
                    }
                }
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
            _reader: reader_thread,
        })
    }

    /// Lock the emulator so the caller can read its screen for rendering.
    pub(crate) fn lock_parser(&self) -> MutexGuard<'_, vt100::Parser> {
        // A panicked reader thread is the only poisoning source; recover the
        // grid anyway so a dead component still renders its last frame.
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
                // One dot, in the separator slot, that blinks by "lighting up"
                // from · to ● (no second dot on the left).
                let dot = if blink_on { "●" } else { "·" };
                format!(" running {dot} {clock} ")
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
    }
}

/// Build the [`CommandBuilder`] that runs `command` through the user's login
/// shell (so aliases, `PATH`, and `$HOME` are as in their terminal).
fn shell_command(command: &str) -> CommandBuilder {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
    let mut cmd = CommandBuilder::new(shell);
    cmd.arg("-lc");
    cmd.arg(command);
    cmd
}

/// Decode a `keys=` attribute into the bytes to send: C-style escapes `\r`,
/// `\n`, `\t`, `\e` (escape, `0x1b`), and `\\` are interpreted; any other `\x`
/// is left as the literal two characters, and a trailing lone `\` is kept.
fn decode_keys(raw: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            let mut buf = [0u8; 4];
            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        match chars.next() {
            Some('r') => out.push(b'\r'),
            Some('n') => out.push(b'\n'),
            Some('t') => out.push(b'\t'),
            Some('e') => out.push(0x1b),
            // An escaped backslash, or a lone trailing one, is a single `\`.
            Some('\\') | None => out.push(b'\\'),
            Some(other) => {
                out.push(b'\\');
                let mut buf = [0u8; 4];
                out.extend_from_slice(other.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
    out
}

/// Owns every running process and applies the per-slide lifecycle.
#[derive(Debug, Default)]
pub(crate) struct ProcessManager {
    /// Ephemeral `tty` processes, keyed by `(slide index, component index)`.
    ephemeral: HashMap<(usize, usize), ManagedProc>,
    /// Persistent `tty` processes, keyed by their `id`.
    persistent: HashMap<String, ManagedProc>,
    /// Prerendered `tty` processes, keyed by `(slide index, component index)`;
    /// spawned at deck load and kept for the whole session.
    prerendered: HashMap<(usize, usize), ManagedProc>,
}

impl ProcessManager {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Apply the lifecycle for navigating to slide `current`: kill ephemeral
    /// processes from other slides and persistent processes whose `id` the new
    /// slide does not reuse. Survivors are re-mounted by [`Self::ensure`].
    /// Prerendered processes are session-lifetime, so they are left untouched.
    ///
    /// Dropping a [`ManagedProc`] kills its child, so `retain` does the killing.
    pub(crate) fn reconcile(&mut self, current: usize, segments: &[Segment]) {
        let keep = keep_persistent_ids(segments);
        self.persistent.retain(|id, _| keep.contains(id));
        self.ephemeral.retain(|(slide, _), _| *slide == current);
    }

    /// Ensure every `tty` component on slide `current` has a process, sized to fit
    /// the `content_rows`x`content_cols` content area minus the box border,
    /// spawning the missing ones and resizing/polling the rest.
    pub(crate) fn ensure(
        &mut self,
        current: usize,
        segments: &[Segment],
        content_rows: u16,
        content_cols: u16,
    ) {
        for (index, segment) in segments.iter().enumerate() {
            let Segment::Tty(component) = segment else {
                continue;
            };
            let (rows, cols) = box_interior(
                component,
                content_rows.saturating_sub(2),
                content_cols.saturating_sub(2),
            );
            let keys = component.keys.as_deref().map(decode_keys);
            let keys = keys.as_deref();
            if component.prerender {
                // Spawned at deck load (see `prerender`); here just resize to the
                // real box and poll. Spawn defensively if load-time spawn was
                // skipped (e.g. a component only reachable after a resize).
                let key = (current, index);
                if let Some(proc) = self.prerendered.get_mut(&key) {
                    proc.resize(rows, cols);
                    proc.poll();
                } else if let Ok(mut proc) = ManagedProc::spawn(&component.command, keys, rows, cols) {
                    proc.poll();
                    self.prerendered.insert(key, proc);
                }
            } else if let Some(id) = persistent_id(component) {
                if let Some(proc) = self.persistent.get_mut(id) {
                    proc.resize(rows, cols);
                    proc.poll();
                } else if let Ok(mut proc) = ManagedProc::spawn(&component.command, keys, rows, cols) {
                    proc.poll();
                    self.persistent.insert(id.to_owned(), proc);
                }
            } else {
                let key = (current, index);
                if let Some(proc) = self.ephemeral.get_mut(&key) {
                    proc.resize(rows, cols);
                    proc.poll();
                } else if let Ok(mut proc) = ManagedProc::spawn(&component.command, keys, rows, cols) {
                    proc.poll();
                    self.ephemeral.insert(key, proc);
                }
            }
        }
    }

    /// Spawn the `prerender` process for one `tty` component at deck load, sized to
    /// an estimate of its eventual box (resized exactly by [`Self::ensure`] when
    /// the slide is shown). A no-op once the process exists; a failed spawn is
    /// simply retried by [`Self::ensure`] on arrival.
    pub(crate) fn prerender(
        &mut self,
        slide: usize,
        index: usize,
        component: &TtyComponent,
        content_rows: u16,
        content_cols: u16,
    ) {
        let key = (slide, index);
        if self.prerendered.contains_key(&key) {
            return;
        }
        let (rows, cols) = box_interior(
            component,
            content_rows.saturating_sub(2),
            content_cols.saturating_sub(2),
        );
        let keys = component.keys.as_deref().map(decode_keys);
        if let Ok(mut proc) = ManagedProc::spawn(&component.command, keys.as_deref(), rows, cols) {
            proc.poll();
            self.prerendered.insert(key, proc);
        }
    }

    /// The process backing a `tty` component on slide `current`, if it spawned.
    pub(crate) fn proc_for(
        &self,
        current: usize,
        index: usize,
        component: &TtyComponent,
    ) -> Option<&ManagedProc> {
        if component.prerender {
            return self.prerendered.get(&(current, index));
        }
        match persistent_id(component) {
            Some(id) => self.persistent.get(id),
            None => self.ephemeral.get(&(current, index)),
        }
    }

}

/// Draw a `<tty>` box: a bordered [`Block`] (title on top, status bar on the
/// bottom) wrapping the process's live emulator screen, or a placeholder if the
/// process failed to start.
pub(crate) fn draw_tty(
    frame: &mut Frame<'_>,
    area: Rect,
    component: &TtyComponent,
    proc: Option<&ManagedProc>,
    blink_on: bool,
) {
    let status = match (&component.status, proc) {
        (Some(fixed), _) => format!(" {fixed} "),
        (None, Some(proc)) => proc.status_line(blink_on),
        (None, None) => " starting… ".to_owned(),
    };
    let bordered = Block::bordered()
        .title(Line::from(format!(" {} ", component.display_title())))
        .title_bottom(Line::from(status).centered());
    match proc {
        Some(proc) => {
            let parser = proc.lock_parser();
            let pseudo = PseudoTerminal::new(parser.screen()).block(bordered);
            frame.render_widget(pseudo, area);
        }
        None => frame.render_widget(Paragraph::new("failed to start").block(bordered), area),
    }
}

/// The box interior for a component: its `rows=`/`cols=` (or defaults), each clamped
/// to the space available and at least 1.
pub(crate) fn box_interior(component: &TtyComponent, max_rows: u16, max_cols: u16) -> (u16, u16) {
    let rows = component
        .rows
        .unwrap_or(DEFAULT_ROWS)
        .min(max_rows.max(1))
        .max(1);
    let cols = component
        .cols
        .unwrap_or(DEFAULT_COLS)
        .min(max_cols.max(1))
        .max(1);
    (rows, cols)
}

/// The persistence key for a component: `Some(id)` only when it both `persist`s and
/// carries an `id` (the link required to survive a slide change).
fn persistent_id(component: &TtyComponent) -> Option<&str> {
    if component.persist {
        component.id.as_deref()
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
            Segment::Tty(component) => persistent_id(component).map(str::to_owned),
            Segment::Markdown(_) | Segment::Code(_) | Segment::Mermaid(_) | Segment::Animation(_) => {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{box_interior, decode_keys, keep_persistent_ids, persistent_id};
    use super::TtyComponent;
    use crate::deck::Segment;

    fn component(id: Option<&str>, persist: bool) -> TtyComponent {
        TtyComponent {
            id: id.map(str::to_owned),
            persist,
            ..TtyComponent::default()
        }
    }

    #[test]
    fn persistent_only_when_persist_and_id_present() {
        assert_eq!(persistent_id(&component(Some("a"), true)), Some("a"));
        assert_eq!(persistent_id(&component(Some("a"), false)), None);
        assert_eq!(persistent_id(&component(None, true)), None);
    }

    #[test]
    fn keep_set_collects_persistent_ids_only() {
        let segments = vec![
            Segment::Markdown("# hi".into()),
            Segment::Tty(component(Some("counter"), true)),
            Segment::Tty(component(Some("scratch"), false)), // not persistent
            Segment::Tty(component(None, true)),             // no id to link
        ];
        let keep = keep_persistent_ids(&segments);
        assert_eq!(keep.len(), 1);
        assert!(keep.contains("counter"));
    }

    #[test]
    fn box_interior_uses_defaults_then_clamps() {
        let wide = TtyComponent {
            cols: Some(200),
            ..TtyComponent::default()
        };
        assert_eq!(box_interior(&wide, 40, 50), (10, 50));

        let small = TtyComponent {
            rows: Some(4),
            cols: Some(20),
            ..TtyComponent::default()
        };
        assert_eq!(box_interior(&small, 40, 80), (4, 20));
    }

    #[test]
    fn decode_keys_interprets_known_escapes() {
        assert_eq!(decode_keys("a\\rb\\n\\t\\e\\\\"), b"a\rb\n\t\x1b\\");
        assert_eq!(decode_keys("\\q"), b"\\q");
        assert_eq!(decode_keys("x\\"), b"x\\");
    }
}
