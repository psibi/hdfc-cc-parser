# ARCHITECTURE.md

## Overview

**HDFC CC Parser** parses HDFC Bank (India) credit card PDF statements into structured CSV data. It extracts transactions (date, description, and amount) from PDF text. Two delivery modes: a CLI binary and a browser-based WebAssembly UI.

## Project Structure

```
hdfc-cc-parser/
├── Cargo.toml              # Root crate (lib + bin)
├── justfile                # Task runner (build, serve, test, release, ci, build-docs)
├── src/
│   ├── lib.rs              # Core parsing logic (lopdf + winnow + manual text extraction)
│   └── main.rs             # CLI binary entry point (clap)
├── cc-parser-ui/
│   ├── Cargo.toml           # UI crate (cdylib for WASM, depends on root crate)
│   ├── Trunk.toml           # Trunk bundler config
│   ├── index.html           # HTML shell with embedded CSS
│   └── src/
│       ├── lib.rs           # WASM entry point (mounts Leptos app)
│       └── app.rs           # Leptos CSR component (upload, password, table, CSV export)
└── pw.pdf                  # Sample input
```

## Data Flow

```
PDF bytes
    │
    ▼
lopdf::Document::load_from    ← parses PDF document structure
    │
    ▼
extract_lines                 ← custom PDF content stream parser (Tj/TJ/Td operators)
    │
    ▼
parse_transactions            ← line-based parser with date detection + amount extraction
    │
    ▼
Vec<Transaction>
    │
    ├── [CLI]  → csv::Writer → transactions.csv
    └── [WASM] → leptos signals → HTML table + Blob download
```

## Core Type

```rust
pub struct Transaction {
    pub date: String,        // dd/mm/yyyy
    pub description: String, // merchant / transaction description
    pub amount: f64,         // negative = debit, positive = credit
}
```

## Parsing Algorithm

The parser operates in two stages:

### Stage 1: PDF Text Extraction (`extract_lines`)

Custom low-level PDF content stream parser that handles:

- **`Tj` operator**: Extracts text from parenthesized strings
- **`TJ` operator**: Extracts text from string arrays (with interleaved positioning)
- **`Td` operator**: Detects line breaks by comparing Y-coordinates (threshold: >3 units difference)

This avoids the `pdf-extract` dependency and handles HDFC's specific PDF layout where text may be split across multiple content stream operations.

### Stage 2: Transaction Parsing (`parse_transactions`)

Line-by-line parsing with the following logic:

1. **Date detection** (`find_date`): Scans each line for the pattern `dd/mm/yyyy | HH:MM` using winnow combinators.
2. **Amount extraction** (`collect_words`): Splits the remainder into words and identifies amounts by:
   - `C` prefix + numeric value → debit (negative)
   - `+` prefix + `C` → credit (positive)
   - Standalone numeric words (with optional `,` separators)
3. **Multi-line descriptions**: If no amount is found on the date line, up to 8 subsequent lines are searched until an amount or next date is found.
4. **Duplicate removal**: Transactions with the same (date, amount) pair are deduplicated using a HashSet.

### Stop Lines

Lines containing any of the following are skipped to avoid parsing headers, summaries, and boilerplate: `Page`, `DATE & TIME`, `TRANSACTION DESCRIPTION`, `Reward Points`, `GST Summary`, `Important Information`, `TOTAL AMOUNT`, `PREVIOUS STATEMENT`, `PAYMENTS/CREDITS`, and others.

## Key Dependencies

| Crate | Purpose |
|---|---|
| `lopdf` | PDF document parsing and content stream extraction |
| `winnow` | Parser combinator framework (date + amount patterns) |
| `clap` (derive) | CLI argument parsing |
| `csv` | CSV serialization |
| `leptos` (csr) | Reactive UI framework — client-side rendering only |
| `wasm-bindgen` / `web-sys` / `js-sys` | Browser API bindings for WASM |

## Build System

- **Cargo workspace** with two crates: the root crate (lib + bin) and `cc-parser-ui` (cdylib + rlib).
- **Trunk** bundles the WASM UI with `index.html` into a static site.
- **`just`** task runner wraps common commands (`just build`, `just serve`, `just test`, `just ci`, `just build-docs`).
- All processing is **client-side only** — no server, no data leaves the browser in the Web UI.
- **`build-docs`** outputs to `docs/` for GitHub Pages deployment at `/hdfc-cc-parser/`.

## Design Decisions

- **Custom PDF text extraction**: Using `lopdf` directly instead of `pdf-extract` gives full control over text extraction and handles HDFC's specific content stream layout (Tj/TJ/Td operators, coordinate-based line breaking).
- **Line-oriented parsing with lookahead**: Each date-bearing line starts a transaction; up to 8 following lines are scanned for amounts, accommodating multi-line descriptions.
- **`C` prefix = debit**: HDFC statements mark debits with a bare `C` before the amount (e.g., `C 1,234.56`). Credits have `+ C` (e.g., `+ C 500.00`).
- **Leptos CSR only (no SSR)**: The Web UI is fully client-side with no server component. The `csr` feature flag is used exclusively.
- **`cdylib` + `rlib` for UI crate**: `cdylib` is required for WASM compilation; `rlib` allows the crate to be used as a Rust library dependency.
- **Password support**: Encrypted PDFs are supported via `parse_pdf_bytes_with_password` using `lopdf::LoadOptions::with_password`.
- **Duplicate filtering**: Lines with identical (date, amount) pairs are removed, as HDFC statements sometimes repeat transaction entries across sections.
