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

use super::{Align, Cached, FrameCtx, Item};
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
    pub(crate) title: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) rows: Option<u16>,
    pub(crate) cols: Option<u16>,
    /// Raw keystrokes (with `\r`/`\n`/`\t`/`\e`/`\\` escapes) fed in after start
    /// to drive a TUI; decoded by [`decode_keys`].
    pub(crate) keys: Option<String>,
    pub(crate) align: Option<Align>,
    pub(crate) command: String,
}

impl TtyComponent {
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
        component.align = super::parse_align(attrs);
        component
    }

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

    fn align(&self) -> Option<Align> {
        self.align
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
            placement: self.placement(),
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

const DEFAULT_ROWS: u16 = 10;
const DEFAULT_COLS: u16 = 80;
const SCROLLBACK: usize = 1000;

/// Cumulative delays (ms) at which a component's `keys=` are (re)sent into the PTY.
/// Spanning a program's async startup means late-appearing popups are caught
/// too, not just whatever is on screen the instant the process spawns.
const KEYS_DELAYS: &[u64] = &[250, 300, 350, 450, 600, 600];

pub(crate) struct ManagedProc {
    /// Fed by the reader thread; locked to read the screen for rendering.
    parser: Arc<Mutex<vt100::Parser>>,
    /// Kept for resizing as the box size changes.
    master: Box<dyn MasterPty + Send>,
    /// Kept for status polling and for `kill` on drop.
    child: Box<dyn Child + Send + Sync>,
    started: Instant,
    /// Frozen elapsed time, set once the exit is observed.
    ended: Option<Instant>,
    status: Option<portable_pty::ExitStatus>,
    rows: u16,
    cols: u16,
    /// Detached; ends on EOF when the child dies.
    _reader: JoinHandle<()>,
}

impl ManagedProc {
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

    pub(crate) fn lock_parser(&self) -> MutexGuard<'_, vt100::Parser> {
        // A panicked reader thread is the only poisoning source; recover the
        // grid anyway so a dead component still renders its last frame.
        self.parser
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Idempotent: freezes the elapsed time and status at the first observed exit.
    fn poll(&mut self) {
        if self.status.is_some() {
            return;
        }
        if let Ok(Some(status)) = self.child.try_wait() {
            self.status = Some(status);
            self.ended = Some(Instant::now());
        }
    }

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

    pub(crate) fn is_running(&self) -> bool {
        self.status.is_none()
    }

    pub(crate) fn interior(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    /// Frozen at exit.
    fn elapsed(&self) -> Duration {
        self.ended.unwrap_or_else(Instant::now) - self.started
    }

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

/// Runs through the user's login shell so aliases, `PATH`, and `$HOME` match
/// their terminal.
fn shell_command(command: &str) -> CommandBuilder {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
    let mut cmd = CommandBuilder::new(shell);
    cmd.arg("-lc");
    cmd.arg(command);
    cmd
}

/// Interprets `\r`/`\n`/`\t`/`\e` (`0x1b`)/`\\`. Any other `\x` is kept as its
/// two literal characters, and a trailing lone `\` is kept.
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
    /// Keyed by `(slide index, component index)`.
    ephemeral: HashMap<(usize, usize), ManagedProc>,
    /// Keyed by `id`.
    persistent: HashMap<String, ManagedProc>,
    /// Keyed by `(slide index, component index)`.
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

    /// Spawn the missing processes for slide `current` and resize/poll the rest.
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

    /// Spawn one `prerender` process at deck load, sized to an estimate of its
    /// eventual box. A no-op once it exists; a failed spawn is retried by
    /// [`Self::ensure`] on arrival.
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

/// `rows=`/`cols=` (or defaults), each clamped to the space available and at
/// least 1.
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

/// `Some(id)` only when the component both `persist`s and carries an `id` — the
/// link required to survive a slide change.
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
            Segment::Markdown(_)
            | Segment::Code(_)
            | Segment::Mermaid(_)
            | Segment::Animation(_)
            | Segment::Image(_) => None,
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
