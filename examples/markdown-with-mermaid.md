# Design Doc with an Embedded Diagram

Markdown often embeds Mermaid diagrams in a fenced ` ```mermaid ` block.
This file is **Markdown**, so `ah-r` renders the prose with termimad and
**draws the diagram below inline** with figurehead.

## Request flow

The login request travels through the gateway before it reaches `Auth`:

```mermaid
graph LR
  Client --> Gateway
  Gateway --> Auth
  Auth --> Database
```

### Steps

1. The client calls the gateway.
2. The gateway forwards to auth.
3. Auth reads from the database.

## Notes

* The prose, headings, and lists render styled by termimad.
* The ` ```mermaid ` block above is drawn as a diagram by figurehead, spliced
  in where its source sits.
* Anything *not* a diagram — **bold**, ~~strike~~, `inline code` — is styled
  the same as in a plain Markdown file.

---

> A ` ```mermaid ` block of a diagram type figurehead can't draw falls back to
> its plain code-block rendering, so nothing is ever lost.
