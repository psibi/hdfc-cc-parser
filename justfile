# default target
default:
    @just --list

# build both CLI and WASM UI (debug)
build: build-cli build-ui

# build only the CLI binary
build-cli:
    cargo build

# build only the WASM UI (debug)
build-ui:
    cd cc-parser-ui && trunk build

# run the CLI on the sample PDF
run:
    cargo run -- pw.pdf

# run CLI with a custom PDF
run-pdf pdf:
    cargo run -- {{pdf}}

# start the UI dev server (hot-reload at localhost:8080)
serve:
    cd cc-parser-ui && trunk serve

# start UI dev server on a custom port
serve-port port:
    cd cc-parser-ui && trunk serve --port {{port}}

# type-check both crates
check:
    cargo check
    cd cc-parser-ui && cargo check --target wasm32-unknown-unknown

# release builds
release: release-cli release-ui

# release CLI binary
release-cli:
    cargo build --release

# release WASM UI (optimized)
release-ui:
    cd cc-parser-ui && trunk build --release

# run clippy on both
lint:
    cargo clippy
    cd cc-parser-ui && cargo clippy --target wasm32-unknown-unknown

# run tests
test:
    cargo test

# CI check (fmt + clippy + tests — deny all warnings)
ci:
    cargo fmt --all -- --check
    cargo fmt --manifest-path cc-parser-ui/Cargo.toml --all -- --check
    cargo clippy --all-targets -- --deny warnings
    cargo clippy --manifest-path cc-parser-ui/Cargo.toml --target wasm32-unknown-unknown -- --deny warnings
    cargo test

# build production site into docs/ (for GitHub Pages)
build-docs:
    cd cc-parser-ui && trunk build --release --dist ../docs --public-url /hdfc-cc-parser/

# clean both build artifacts
clean:
    cargo clean
    cd cc-parser-ui && cargo clean
    rm -rf cc-parser-ui/dist
    rm -rf docs
