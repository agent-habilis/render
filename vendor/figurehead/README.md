# Figurehead ![CI](https://github.com/Mootikins/figurehead-rs/actions/workflows/ci.yml/badge.svg?branch=master) [![crates.io](https://img.shields.io/crates/v/figurehead.svg)](https://crates.io/crates/figurehead) [![docs.rs](https://docs.rs/figurehead/badge.svg)](https://docs.rs/figurehead)

A Rust utility to convert Mermaid.js diagram markup into ASCII diagrams, inspired by [mermaid-ascii](https://github.com/AlexanderGrooff/mermaid-ascii).

_Ideas:_ add a small banner/screenshot of a rendered flowchart once we have more diagram types; a light/dark pair of ASCII captures would fit the theme. Swap the crates.io/docs.rs badges to live ones after publishing.

## Features

- 🎨 Convert Mermaid.js flowcharts to ASCII art
- 🔧 Modular, plugin-based architecture inspired by mermaid.js
- 🚀 Fast parsing using [chumsky](https://github.com/zesterer/chumsky)
- 🧪 Test-Driven Development approach
- 🌐 WASM-compatible core library (future browser support)
- 🎯 SOLID principles and Rust idioms

## Architecture

Figurehead follows a modular pipeline architecture:

```
Input → Detector → Parser → Database → Layout → Renderer → Output
```

### Core Components

- **Detector**: Identifies diagram types from markup patterns
- **Parser**: Parses markup into structured data using chumsky
- **Database**: Stores diagram nodes and edges
- **Layout**: Arranges elements in coordinate space (Dagre-inspired)
- **Renderer**: Generates ASCII output

### Diagram Types

Currently supported:
- Flowchart with subgraphs (single-level nesting)
- Git graphs
- Sequence diagrams
- Class diagrams (with relationships)

Planned:
- Nested subgraphs
- Styling with terminal colors

## Quick Start

For layout/rendering review targets, see `docs/ideal-output/`.

### CLI Usage

```bash
# Basic conversion
echo "graph TD\n    A --> B" | figurehead

# Choose output character set (ascii|unicode|unicode-math|compact)
figurehead convert --style ascii -i input.mmd

# Output to file
figurehead -i input.mmd -o output.txt

# Respect FIGUREHEAD_STYLE environment variable
FIGUREHEAD_STYLE=compact figurehead convert -i input.mmd

# Enable debug logging
figurehead convert --log-level debug --log-format pretty -i input.mmd

# Use environment variables for logging
FIGUREHEAD_LOG_LEVEL=debug FIGUREHEAD_LOG_FORMAT=json figurehead convert -i input.mmd
```

### Library Usage

```rust
use figurehead::{plugins::flowchart::FlowchartDiagram, core::Diagram};

// Create a flowchart diagram
let diagram = FlowchartDiagram;

// Create parser and database
let mut parser = diagram.create_parser();
let mut database = diagram.create_database();

// Parse some markup
parser.parse("graph TD\n    A --> B", &mut database)?;

// Render to ASCII
let renderer = diagram.create_renderer();
let output = renderer.render(&database)?;
println!("{}", output);
```

### Subgraph Support

Figurehead supports single-level subgraphs to group related nodes:

```bash
echo 'graph LR
    subgraph "Backend"
        API --> DB
    end
    Client --> API' | figurehead convert -i -
```

Output:
```
            ┌── Backend ───┐
┌────────┐  │ ┌─────┐ ┌────┐ │
│ Client │──┼▶│ API │─▶│ DB │ │
└────────┘  │ └─────┘ └────┘ │
            └────────────────┘
```

**Limitations:**
- Only single-level nesting is supported (nested subgraphs are flattened)
- Subgraph borders may overlap if their nodes are not visually separated

## Logging

Figurehead includes comprehensive structured logging using the `tracing` crate.
This helps with debugging diagram processing and understanding performance.

### Log Levels

- `trace`: Very detailed information for deep debugging
- `debug`: Detailed information for debugging (recommended for development)
- `info`: General informational messages (default)
- `warn`: Warning messages
- `error`: Error messages only

### Log Formats

- `compact`: Single-line format, good for production (default)
- `pretty`: Multi-line format with colors, good for development
- `json`: JSON format, good for log aggregation systems

### Usage Examples

```bash
# Enable debug logging with pretty format
figurehead convert --log-level debug --log-format pretty -i input.mmd

# Use environment variables (overrides CLI flags)
FIGUREHEAD_LOG_LEVEL=trace FIGUREHEAD_LOG_FORMAT=json figurehead convert -i input.mmd

# Filter logs by component
RUST_LOG="figurehead::plugins::flowchart::parser=debug" figurehead convert -i input.mmd
```

### Debugging with Logs

When debugging diagram processing issues, enable debug logging to see:

- **Detection**: Which diagram type was detected and confidence scores
- **Parsing**: Parsing stages, node/edge counts, and any skipped statements
- **Layout**: Layer assignment, node positioning, edge routing, and canvas dimensions
- **Rendering**: Canvas creation, nodes/edges drawn, and final output size

Example output with `--log-level debug --log-format pretty`:

```
INFO  figurehead::plugins::orchestrator: Starting diagram processing pipeline
DEBUG figurehead::plugins::orchestrator: Diagram type detected diagram_type=flowchart
DEBUG figurehead::plugins::flowchart::parser: Parsing completed node_count=3 edge_count=2
DEBUG figurehead::plugins::flowchart::layout: Layout completed node_count=3 edge_count=2 width=45 height=12
DEBUG figurehead::plugins::flowchart::renderer: Rendering completed output_len=234 canvas_width=45 canvas_height=12
INFO  figurehead::plugins::orchestrator: Pipeline completed successfully
```

## Development

### Building

```bash
cargo build
cargo test
```

### Running Tests

```bash
# All tests
cargo test

# Core trait tests
cargo test --test core_traits

# Run with colors
cargo test -- --nocapture

# Run with logging enabled
RUST_LOG=debug cargo test
```

### Project Structure

```
src/
├── core/           # Core trait abstractions
├── plugins/        # Diagram type implementations
│   └── flowchart/  # Flowchart plugin
├── layout/         # Layout algorithms
├── rendering/      # Rendering implementations
└── cli/           # CLI interface
tests/
└── core_traits.rs  # Core functionality tests
```

## WASM Support

The core library is fully WASM-compatible and includes a web example:

### Building WASM Module

```bash
# Install wasm-pack if needed
cargo install wasm-pack

# Build for web
cd examples/web
./build.sh
```

### Running Web Example

```bash
# Build WASM module
cd examples/web
./build.sh

# Serve with Rust HTTP server
cargo run --bin server
# Then visit http://localhost:8000/
```

The web examples provide:
- **Interactive Editor** (`index.html`): Full-featured editor with examples
- **Live Editor** (`editor.html`): Minimal split-pane editor with real-time updates
- Multiple character set styles
- Parse-only mode to inspect diagram structure

See `examples/web-editor/README.md` for more details.

## Dependencies

- [chumsky](https://github.com/zesterer/chumsky) - Parsing library
- [clap](https://github.com/clap-rs/clap) - CLI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal handling
- [anyhow](https://github.com/dtolnay/anyhow) - Error handling
- [thiserror](https://github.com/dtolnay/thiserror) - Error types
- [unicode-width](https://github.com/unicode-rs/unicode-width) - Text width calculation

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

See [AGENTS.md](AGENTS.md) for development guidelines.

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Acknowledgments

- Inspired by [mermaid-ascii](https://github.com/AlexanderGrooff/mermaid-ascii)
- Architecture inspired by [mermaid.js](https://github.com/mermaid-js/mermaid)
- Built with the Rust ecosystem
