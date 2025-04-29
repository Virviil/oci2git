# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands
- Build: `cargo build`
- Run: `cargo run`
- Test all: `cargo test`
- Test single test: `cargo test test_name`
- Format code: `cargo fmt`
- Lint code: `cargo clippy`
- Fix linting issues: `cargo clippy --fix`
- Documentation: `cargo doc`

## Code Style Guidelines
- Use 2018 edition Rust standards
- Format code with rustfmt (standard settings)
- Use Clippy for linting
- Imports: Group standard library imports first, then external crates, then local modules
- Types: Use strong typing with proper error handling patterns
- Error handling: Use Result<T, E> for functions that can fail
- Naming: Follow Rust conventions (snake_case for variables/functions, CamelCase for types)
- Comments: Document public API with rustdoc /// style comments
- Maximum line length: 100 characters