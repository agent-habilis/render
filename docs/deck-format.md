# The `.ahr-deck` deck format

An `.ahr-deck` deck is a **Markdown file with embedded XML islands**. `ah-r` renders
it as a full-screen terminal presentation: each slide is a vertically-centered
stack of Markdown prose and embedded components (code, diagrams, live shells,
animations).

```sh
ah-r slides.ahr-deck
```

A deck takes over the terminal, so it needs an **interactive TTY** on both
stdin and stdout — in a pipe or redirect `ah-r` errors instead of rendering.
Navigate with the arrow keys; press `q` to quit.

| Action | Keys |
| --- | --- |
| Next slide | `→` `↓` `PageDown` `Space` `l` `j` `n` |
| Previous slide | `←` `↑` `PageUp` `Backspace` `h` `k` `p` |
| First / last slide | `Home` `g` / `End` `G` |
| Quit | `q` `Esc` `Ctrl-C` |

## Live reload (`--watch`)

`ah-r slides.ahr-deck --watch` watches the file and re-renders on every save, so you
can edit and preview in a tight loop. It keeps your place (the current page) and
leaves any running `<tty>` processes alive across the reload. If the page you're
on no longer exists — e.g. you deleted the last few slides — it drops to the new
last slide. While the file is mid-edit and doesn't parse (an unclosed tag, two
`<deck>` tags), it shows the parse error and resumes once the file is valid
again. Off unless `--watch` is given.

## Slides

Slides are separated by a standalone `---` line:

```text
# First slide

---

# Second slide
```

Splitting is **fence- and component-aware**: a `---` inside a Markdown code
fence or inside a component (e.g. a `<code>` body or an `<animation>` frame) is
content, not a separator. A deck must contain at least one slide.

## Frontmatter

Optional metadata is a self-closing `<deck>` tag at the **start** of a slide
(after any blank lines); unknown attributes are ignored.

- `<deck title="…" />` — the footer title, shown on every slide. A `<deck>` with
  no slide body of its own is metadata only, not a slide. **At most one `<deck>`
  per document** — a second one is an error.
- `pagination="…"` — a template for the footer's page indicator. `{cur}` and
  `{total}` expand to the current page (1-based) and the slide count; `{{`/`}}`
  are literal braces. Omit it for the default `current/total`.
- `progress="false"` — turn off the terminal progress indicator. While a deck is
  open `ah-r` reports the current slide of the total via the **OSC 9;4** escape,
  so terminals that support it (Ghostty, WezTerm, kitty, Windows Terminal,
  ConEmu) show a progress bar in the tab/taskbar; it's cleared on quit. On by
  default — `progress="false"` (or the bare `progress` flag to force it on).

Both `title` and `pagination` are rendered **as-is**, so they can carry ANSI
colors and Nerd Font glyphs — e.g. a powerline pill (see `statusline.ahr-deck` and
`typography.ahr-deck`).

```text
<deck title="My Talk" pagination=" {cur}/{total} " />

# Intro

---

# The details
```

The footer shows the title on the left and the page indicator on the right (no
rule line above it).

## Default content: Markdown

Anything that isn't frontmatter or a component is ordinary Markdown, styled for
the terminal (headings, lists, blockquotes, inline code, tables, width-wrapped).

A plain ` ``` ` code fence renders as a styled (non-highlighted) block. Note a
` ```mermaid ` fence inside slide Markdown is **just a styled code block** — to
draw a diagram use the `<mermaid>` component below.

## Components

Components are XML islands. An opening tag must sit on its own line (trimmed),
the whole tag on that one line ending in `>`; the closing tag is `</name>` on
its own line (or the tag self-closes with `/>`, like `<img … />`). A tag that
never closes is left as plain Markdown, and a `<code>`-looking line inside an
opaque ` ``` ` fence is not parsed as a component.

### A global attribute: `align`

Every component takes `align="left | center | right"`, controlling its horizontal
placement on the slide. The default is centered, except `<code>` (and prose),
which defaults to the shared flush-left text column; an explicit `align` overrides
that. `left` and `right` hug the edges of the centered content column. See
[`examples/align.ahr-deck`](../examples/align.ahr-deck).

### `<code lang="…">` — syntax-highlighted code

Renders its body with static syntax highlighting (Tokyo Night Moon theme),
borderless. `lang` selects the grammar (`rust`, `html`, `js`, `ts`, `go`,
`yaml`, `sh`, …); an unknown or missing `lang` falls back to plain text.

```text
<code lang="rust">
fn main() {
    println!("hi");
}
</code>
```

### `<mermaid>` — a drawn diagram

Draws its body as a Unicode diagram (flowchart, sequence, class, gitgraph, …).
If it can't be drawn, the raw source is shown instead.

```text
<mermaid>
flowchart LR
  A --> B --> C
</mermaid>
```

### `<tty …>` — a live process

Runs the body as a shell command/script in a real pseudo-terminal, drawn live
in a bordered box that streams output and shows a status line (a blinking
running indicator with elapsed time, then the exit mark and code).

```text
<tty title="clock" rows="6" cols="46">
while :; do date "+%H:%M:%S"; sleep 1; done
</tty>
```

| Attribute | Meaning |
| --- | --- |
| `id` | Links the same logical process across slides (used with `persist`). |
| `persist` | Keep the process alive across a slide change when the next slide has a `<tty>` with the same `id` (so it isn't restarted). |
| `prerender` | Spawn at deck load and keep alive for the whole session, so a slow program is warm when its slide is reached. Independent of `persist`/`id`. |
| `title` | Box title; defaults to the command's first token. |
| `status` | Fixed status text; defaults to the live running/exited status. |
| `rows` / `cols` | Box interior size in cells (defaults 10 × 80), clamped to fit. |
| `keys` | Keystrokes fed into the process shortly after it starts, to drive a TUI (e.g. dismiss an editor popup). Raw, with `\r` `\n` `\t` `\e` `\\` escapes. |

`persist` / `prerender` are bare flags (write `persist="false"` to disable).

### `<animation …>` — a flipbook

Cycles through literal `<frame>` children, advancing every `seconds`. Frames are
preformatted and verbatim — every space is kept, so figures stay aligned across
the flip.

```text
<animation seconds="0.12" loop>
<frame>
   |
</frame>
<frame>
   /
</frame>
</animation>
```

- `seconds` — per-frame duration (default `1`, clamped to `[0.05, 3600]`).
- `loop` — bare flag; wrap forever. Without it the animation plays once and
  holds the last frame.

### `<img … />` — an image

Draws an image file with the terminal's graphics protocol — kitty, iTerm2, or
Sixel for true pixels, falling back to Unicode halfblocks on terminals that
support none. It is a **void tag** (no body); the trailing `/` is optional, and
the image is centered on its own row.

```text
<img src="assets/logo.png" width="40" />
```

| Attribute | Meaning |
| --- | --- |
| `src` | Path to the image, resolved against the deck file's directory (absolute paths are used as-is). |
| `width` / `height` | Target display size in cells, scaling the image up or down to it. Aspect ratio is always preserved: give one and the other is derived; give both to fit within that box; give neither for the natural size. Clamped to the slide so it never overflows. |
| `alt` | Text drawn in place of the image when the file can't be loaded, so a broken path never blanks the slide. |

The graphics protocol is detected at deck load by querying the terminal. Set
`AH_R_NO_IMAGE_QUERY=1` to skip that query (forcing the halfblock fallback) on
terminals where it misbehaves.

## A minimal deck

```text
<deck title="Hello" />

# Hello, deck

Some **Markdown**, then a diagram:

<mermaid>
flowchart LR
  idea --> render[ah-r] --> terminal
</mermaid>

---

## Live

<tty title="clock" rows="4" cols="30">
date
</tty>
```

See [`examples/`](../examples/) for complete decks: `showcase.ahr-deck`,
`animation.ahr-deck`, `demo.ahr-deck`, `nvim.ahr-deck`, `statusline.ahr-deck`,
`image.ahr-deck`, `align.ahr-deck`.
