# Repository Guidelines

## Project Structure & Module Organization

`mdbijou` is a Rust 2021 native Markdown reader/editor for macOS. The binary entry point is `src/main.rs`; keep application state and keyboard/file workflows in `src/app.rs`. Parsing belongs in `src/document.rs`, preview rendering in `src/render.rs`, editing and syntax highlighting in `src/editor.rs` and `src/highlight.rs`, image loading/caching in `src/images.rs`, and reusable theme, configuration, and font behavior in their matching modules. Shell helpers are in `scripts/`, while `sample.md` is the local smoke-test fixture. Generated output stays under `target/` and must not be committed. The `docs/` directory is local-only scratch space: it is ignored by git and must not be used as a source of reference or authority for code, behavior, or decisions.

## Local Skills

- `skills/changelog-release-notes/SKILL.md` — MUST use before submitting any PR that includes user-visible changes. Follow it to update `CHANGELOG.md`, release notes, or PR descriptions. Always verify the latest shipped beta and add `_(PR [#...] by [@...])_` credit for every PR-derived changelog item.

## Build, Test, and Development Commands

Rust 1.85+ and `just` are the expected tools.

- `just` builds and opens `sample.md` in a debug run.
- `just check` performs a fast compile check.
- `just clippy` lints all targets and treats warnings as errors.
- `just fmt` formats Rust sources with `rustfmt`.
- `cargo test` runs the test suite.
- `just release` creates the size-optimized binary at `target/release/mdbijou`.
- `just icon` regenerates the macOS-style squircle master `assets/mdbijou-icon-1024.png` (`scripts/icon-compose.py`, needs Pillow) and `assets/mdbijou.icns` from `logo.png` (sips + iconutil).
- `just bundle` packages `dist/mdbijou.app` (release binary + icon + Info.plist + ad-hoc codesign); `just dmg` also produces a `.dmg` in `dist/`.
- `just baseline` records release size and startup sanity results.

Use `just edit path/to/file.md` to exercise editing and `just preview path/to/file.md` to inspect rendering.

## Coding Style & Naming Conventions

Follow standard `rustfmt` output (four-space indentation). Use `snake_case` for modules, functions, and variables; `PascalCase` for structs and enums; and `SCREAMING_SNAKE_CASE` for constants. Keep feature-dependent behavior behind the existing Cargo features (`highlight`, `editor`, `lite-highlight`, and `remote-images`). Prefer small, focused modules and propagate recoverable errors instead of panicking in file, configuration, or rendering paths.

## egui Text Vertical Positioning

In egui, a rect/galley's geometric center is NOT the glyphs' optical center: galley bounds come from font metrics (ascent/descent, `line_height`), and the default valign is `Align::BOTTOM`. With CJK fallback fonts in the stack (PingFang's ascent ≈ 1.16em) the box extends far beyond the visible ink, so `painter.text(rect.center(), Align2::CENTER_CENTER, …)` renders text visibly too high, and `TextFormat.strikethrough` (drawn at the glyph *logical* box center) lands below the visual middle. Therefore:

- When vertically centering text in a rect, correct by the ink bounds (`placed.row.visuals.mesh_bounds`), as `paint_optical_centered_text` in `src/app.rs` does.
- Never use `TextFormat.strikethrough`; record spans and draw the line at the row's ink center, as `paint_job`/`StrikeSpan` in `src/render.rs` do.
- After touching such code, eyeball the result at several font sizes — metrics shift with the font family/size chosen in settings.

## Testing Guidelines

Unit tests live beside the implementation in `#[cfg(test)] mod tests` (for example the parser tests in `src/document.rs` and the image resolution tests in `src/images.rs`), and `cargo test` currently passes. Add integration tests under `tests/` when behavior crosses module or CLI boundaries. Name tests by observable behavior, for example `preserves_task_list_state`. Before submitting, run `cargo test`, `just clippy`, `just check`, and manually open `sample.md`. Changes affecting release size or startup should also run `just baseline` and note the result.

## Commit & Pull Request Guidelines

History currently contains only an initial descriptive commit, so no strict convention is established. Use concise, imperative subjects such as `Fix relative image resolution`, and keep each commit scoped to one concern. All git commit messages and pull request titles/descriptions must be written in English. Before opening a PR, (1) if it includes user-visible changes, update `CHANGELOG.md` per `skills/changelog-release-notes/SKILL.md`: verify the latest shipped `vX.Y.Z-beta.N` (`git fetch --tags` + `git tag --sort=-v:refname | rg '^v[0-9]+\.[0-9]+\.[0-9]+-beta\.' | head`), add entries under `vX.Y.Z-beta.(N+1)` grouped as `Added`/`Changed`/`Fixed` and end every PR-derived bullet with `_(PR [#...] by [@...])_`; (2) assess postmortem per `## Postmortems` and state the result in the PR description (`postmortem/YYYY-MM-DD-title.md` linked or `No postmortem required: [reason]`). Pull requests should explain the user-visible change, list validation performed, link relevant issues, and include screenshots for rendering, theme, or editor UI changes. Call out feature-flag, platform, binary-size, or startup-time effects explicitly.

## Branching

- `main` — stable
- Feature branches: `feat/<short-name>`
- Fix branches: `fix/<short-name>`

## Postmortems

For every PR that fixes a bug, regression, or production issue, assess whether a postmortem is required — this check is mandatory on every PR. MUST create `postmortem/YYYY-MM-DD-title.md` when any of these holds: regression from a prior shipped beta, crash / data-loss / security issue, user-visible breakage, or investigation/debugging > ~4 hours. Template:

- What happened
- Root cause
- Fix applied
- What we learned (prevention, test, or process change)

Link the postmortem file in the PR description. If no postmortem is needed, explicitly state `No postmortem required: [reason, e.g., trivial typo / single-line fix without regression]` in the PR description so the check is auditable.