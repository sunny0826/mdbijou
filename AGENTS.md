# Repository Guidelines

## Project Structure & Module Organization

`mdbijou` is a Rust 2021 native Markdown reader/editor for macOS. The binary entry point is `src/main.rs`; keep application state and keyboard/file workflows in `src/app.rs`. Parsing belongs in `src/document.rs`, preview rendering in `src/render.rs`, editing and syntax highlighting in `src/editor.rs` and `src/highlight.rs`, and reusable theme, configuration, and font behavior in their matching modules. Shell helpers are in `scripts/`, while `sample.md` is the local smoke-test fixture. Generated output stays under `target/` and must not be committed. The `docs/` directory is local-only scratch space: it is ignored by git and must not be used as a source of reference or authority for code, behavior, or decisions.

## Build, Test, and Development Commands

Rust 1.85+ and `just` are the expected tools.

- `just` builds and opens `sample.md` in a debug run.
- `just check` performs a fast compile check.
- `just clippy` lints all targets and treats warnings as errors.
- `just fmt` formats Rust sources with `rustfmt`.
- `cargo test` runs the test suite.
- `just release` creates the size-optimized binary at `target/release/mdbijou`.
- `just baseline` records release size and startup sanity results.

Use `just edit path/to/file.md` to exercise editing and `just preview path/to/file.md` to inspect rendering.

## Coding Style & Naming Conventions

Follow standard `rustfmt` output (four-space indentation). Use `snake_case` for modules, functions, and variables; `PascalCase` for structs and enums; and `SCREAMING_SNAKE_CASE` for constants. Keep feature-dependent behavior behind the existing Cargo features (`highlight`, `editor`, `lite-highlight`, and `remote-images`). Prefer small, focused modules and propagate recoverable errors instead of panicking in file, configuration, or rendering paths.

## Testing Guidelines

The repository currently has no committed automated tests. Add unit tests beside the implementation in `#[cfg(test)] mod tests`; add integration tests under `tests/` when behavior crosses module or CLI boundaries. Name tests by observable behavior, for example `preserves_task_list_state`. Before submitting, run `cargo test`, `just clippy`, `just check`, and manually open `sample.md`. Changes affecting release size or startup should also run `just baseline` and note the result.

## Commit & Pull Request Guidelines

History currently contains only an initial descriptive commit, so no strict convention is established. Use concise, imperative subjects such as `Fix relative image resolution`, and keep each commit scoped to one concern. All git commit messages and pull request titles/descriptions must be written in English. Pull requests should explain the user-visible change, list validation performed, link relevant issues, and include screenshots for rendering, theme, or editor UI changes. Call out feature-flag, platform, binary-size, or startup-time effects explicitly.
