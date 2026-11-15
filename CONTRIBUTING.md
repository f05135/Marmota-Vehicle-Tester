# Contributing to Marmota Vehicle Tester

Thank you for your interest in contributing!

## Development Setup

1. Install [Rust](https://rustup.rs/) (`1.80+`, Edition 2021).
2. Clone the repository and run in development mode:
   ```bash
   cargo run
   ```
3. Before opening a Pull Request, make sure formatting and tests pass:
   ```bash
   cargo fmt --check
   cargo test
   ```

## Guidelines
- Keep all code comments, commit messages, and documentation in English.
- Preserve offline-first behavior and avoid adding unnecessary runtime dependencies.
- Never commit personal paths, API keys, local `data/` databases, or `target/` build artifacts.
