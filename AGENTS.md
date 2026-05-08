# Repository Guidelines

## Project Structure & Module Organization

`chidori` is a Rust CLI for fetching web pages and converting readable content to Markdown. Core source lives in `src/`: `main.rs` wires the binary, `cli.rs` defines arguments, and modules such as `fetcher.rs`, `extractor.rs`, `cleaner.rs`, `metadata.rs`, `markdown.rs`, and `output.rs` own the pipeline stages. Integration tests live in `tests/`, with HTML regression inputs under `tests/fixtures/` and additional real-world extraction cases in nested fixture directories. `bin/chidori.js` is the npm package entrypoint that launches the compiled Rust binary.

## Build, Test, and Development Commands

- `npm run build` or `cargo build --release`: build the release CLI.
- `cargo run -- https://example.com`: run the CLI locally while developing.
- `npm test` or `cargo test`: run unit and integration tests.
- `npm run format`: apply `rustfmt` to all Rust code.
- `npm run format:check`: verify formatting without changing files.
- `npm run lint`: run Clippy for all targets and features with warnings denied.
- `npm run precommit`: run the formatting check and linter before submitting work.

## Coding Style & Naming Conventions

Use Rust 2021 conventions and let `cargo fmt` decide indentation and layout. Keep module names lowercase with underscores when needed, and use descriptive function names such as `extract_metadata` or `clean_html`. Prefer small pipeline-oriented functions that return `Result<T, ChidoriError>` for fallible behavior. For code search, use `rg` for text and `ast-grep` for syntax-aware Rust, TypeScript, or JSX patterns.

## Testing Guidelines

Add focused unit tests beside the module when behavior is local, and integration tests in `tests/` when CLI behavior or cross-module extraction changes. Use `assert_cmd`, `predicates`, `wiremock`, and fixtures consistently with existing tests. Add or update HTML fixtures for parser regressions, especially extraction, cleanup, metadata, JSON-LD, and Markdown formatting cases. Run `cargo test` before opening a PR.

## Commit & Pull Request Guidelines

Recent history uses concise Conventional Commit-style messages, for example `fix: accept parameterized json ld script types` and `feat: add schema-backed extraction`. Keep commit messages in English. PRs should include a short behavior summary, tests run, linked issues when applicable, and before/after examples for user-visible CLI output changes. Include fixture names or command output snippets when they clarify extraction regressions.

## Security & Configuration Tips

Do not commit secrets, credentials, or downloaded private pages as fixtures. Keep network-related tests deterministic by using local fixtures or `wiremock`. Preserve the CLI contract: Markdown or JSON goes to stdout, while logs and errors go to stderr.
