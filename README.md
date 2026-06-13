# HDFC CC Parser

Parse HDFC Bank credit card PDF statements into CSV transactions. Works as a CLI tool and a browser-based WebAssembly UI — all processing is client-side, no data leaves your machine.

## Quick Start

```bash
# Build both CLI and WASM UI
just build

# Parse a PDF via CLI
just run

# Start the browser UI (hot-reload at localhost:8080)
just serve
```

## Usage

**CLI:**

```bash
cargo run -- statement.pdf
# outputs transactions.csv
```

**Browser UI:** Open `just serve`, drag-and-drop a PDF, view the table, and download CSV.

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for design details.
