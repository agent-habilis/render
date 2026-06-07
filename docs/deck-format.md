# The `.ah-r` deck format

An `.ah-r` deck is a **Markdown file with embedded XML islands**. `ah-r` renders
it as a full-screen terminal presentation: each slide is a vertically-centered
stack of Markdown prose and embedded components (code, diagrams, live shells,
animations).

```sh
ah-r slides.ah-r
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

Optional metadata is a self-closing tag at the **start** of a slide (after any
blank lines). Only the `title` attribute is read; any others are ignored.

- `<deck title="…" />` — deck-level. Place it at the top of the file; its
  `title` is the default footer title for every slide. A `<deck>` with no slide
  body of its own is metadata only, not a slide. **At most one `<deck>` per
  document** — a second one is an error.
- `<slide title="…" />` — per-slide. Overrides the deck title in that slide's
  footer.

```text
<deck title="My Talk" />

# Intro

---

<slide title="Details" />

# The details
```

The footer shows the active title on the left and `current/total` on the right.

## Default content: Markdown

Anything that isn't frontmatter or a component is ordinary Markdown, styled for
the terminal (headings, lists, blockquotes, inline code, tables, width-wrapped).

A plain ` ``` ` code fence renders as a styled (non-highlighted) block. Note a
` ```mermaid ` fence inside slide Markdown is **just a styled code block** — to
draw a diagram use the `<mermaid>` component below.

## Components

Components are XML islands. An opening tag must sit on its own line (trimmed),
the whole tag on that one line ending in `>`; the closing tag is `</name>` on
its own line. A tag that never closes is left as plain Markdown, and a
`<code>`-looking line inside an opaque ` ``` ` fence is not parsed as a
component.

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

See [`examples/`](../examples/) for complete decks: `showcase.ah-r`,
`animation.ah-r`, `demo.ah-r`, `nvim.ah-r`.
