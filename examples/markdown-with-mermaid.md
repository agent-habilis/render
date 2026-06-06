# Design Doc with an Embedded Diagram

Markdown often embeds Mermaid diagrams in a fenced ` ```mermaid ` block.
This file is **Markdown**, so `ahr` renders the prose with termimad and
**draws the diagram below inline** with figurehead.

## Request flow

The login request travels through the gateway before it reaches auth:

```mermaid
graph LR
  Client --> Gateway
  Gateway --> Auth
  Auth --> Database
```

## Notes

- The prose, headings, and list render styled by termimad.
- The ` ```mermaid ` block above is drawn as a diagram by figurehead, spliced
  in where its source sits.

> A ` ```mermaid ` block of a diagram type figurehead can't draw falls back to
> its plain code-block rendering, so nothing is ever lost.
