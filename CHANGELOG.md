# Changelog

## v0.0.3 (unreleased)

### Added

- MDX: open, preview, edit, and save `.mdx` documents; register them with the macOS app; and render frontmatter titles, responsive `CardGroup`/`Card` grids, `Steps`/`Step` sequences, common semantic HTML, and Mermaid fences with MDX properties without executing JSX or scripts _(PR [#14](https://github.com/sunny0826/mdbijou/pull/14) by [@sunny0826](https://github.com/sunny0826))_
- CLI: reject nonexistent file paths with an error and nonzero exit status before launching the app _(PR [#15](https://github.com/sunny0826/mdbijou/pull/15) by [@sunny0826](https://github.com/sunny0826))_

## v0.0.2

### Changed

- Process: enforce `CHANGELOG.md` update and postmortem assessment on every PR via `AGENTS.md` _(PR [#12](https://github.com/sunny0826/mdbijou/pull/12) by [@sunny0826](https://github.com/sunny0826))_

### Fixed

- Status bar: remove duplicate filename (already shown in title bar) and fix word-count `T` icon rendering smaller than adjacent text _(PR [#12](https://github.com/sunny0826/mdbijou/pull/12) by [@sunny0826](https://github.com/sunny0826))_
- Preview: fix table horizontal scrollbar not reaching the last column and trailing blank when scrolled to end _(PR [#13](https://github.com/sunny0826/mdbijou/pull/13) by [@sunny0826](https://github.com/sunny0826))_

## v0.0.1

First public release: a lightweight native macOS Markdown reader + simple editor. Native GUI (egui/eframe), no webview, small binary and fast startup; CJK-friendly (auto-loads PingFang SC).

### Added

**macOS**
- Markdown rendering: CommonMark + GFM (tables, task lists, strikethrough, footnotes), with a full IR parse + preview rendering pipeline for lists, tables, links, HTML, images, and the scrollbar
  _(PR [#1](https://github.com/sunny0826/mdbijou/pull/1) by [@sunny0826](https://github.com/sunny0826/mdbijou))_
- Native integration: traffic-light toolbar, settings page (`Cmd+,`), and one-click install of the `mdb` CLI to PATH
  _(PR [#2](https://github.com/sunny0826/mdbijou/pull/2) by [@sunny0826](https://github.com/sunny0826/mdbijou))_
- Packaging & file opening: `.app`/`.dmg` packaging (with icon + ad-hoc signing), open `.md` via Finder double-click/drag (`application:openFiles:`), Mermaid diagram rendering, font & font-size settings, and optical text centering
  _(PR [#3](https://github.com/sunny0826/mdbijou/pull/3) by [@sunny0826](https://github.com/sunny0826/mdbijou))_
- UI polish: design tokens, status bar, themed feedback colors, and refined settings controls
  _(PR [#4](https://github.com/sunny0826/mdbijou/pull/4) by [@sunny0826](https://github.com/sunny0826/mdbijou))_
- Themes: `github-light` / `github-dark` / `sepia`, switching applies instantly
- Unsaved-changes guard: save/discard/cancel confirmation before opening a new file

**Editor**
- `Cmd+E` edit/preview toggle, `Cmd+S` save, and syntax highlighting for full Markdown plus fenced code blocks (syntect)

### Changed

- Release build is ~**7.2 MB**; cold startup (spawn-to-alive) is ~**16–22 ms** (Apple Silicon)

### Fixed

- Markdown IR parse and preview rendering issues with lists, tables, links, HTML, images, and the scrollbar
  _(PR [#1](https://github.com/sunny0826/mdbijou/pull/1) by [@sunny0826](https://github.com/sunny0826/mdbijou))_
