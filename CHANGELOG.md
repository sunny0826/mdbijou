# Changelog

## v0.0.4 (unreleased)

### Added

- macOS: keyboard-shortcut regression tests simulate real ⌘-key dispatch through the focus path, so shortcut regressions now fail `cargo test` instead of shipping silently _(PR [#18](https://github.com/sunny0826/mdbijou/pull/18) by [@sunny0826](https://github.com/sunny0826))_
- macOS: the Settings window closes with `Esc`, and the Dock icon now shows the app artwork in dev builds too (previously only packaged builds had an icon) _(PR [#18](https://github.com/sunny0826/mdbijou/pull/18) by [@sunny0826](https://github.com/sunny0826))_
- macOS: preview code blocks now show a language header bar with syntax highlighting (comments, strings, numbers, keywords, functions, types, operators) colored from the active theme's palette _(PR [#17](https://github.com/sunny0826/mdbijou/pull/17) by [@sunny0826](https://github.com/sunny0826))_

### Changed

- macOS: the Settings window is redesigned in the macOS System Settings style — borderless inset groups, a single row of compact theme tiles with live palette previews, tighter rows and steppers; the follow-system-theme toggle was removed _(PR [#18](https://github.com/sunny0826/mdbijou/pull/18) by [@sunny0826](https://github.com/sunny0826))_
- macOS: toolbar buttons use the official Lucide icon set for a consistent 24-px stroke look _(PR [#18](https://github.com/sunny0826/mdbijou/pull/18) by [@sunny0826](https://github.com/sunny0826))_
- macOS: preview typography polish — headings scale with the reader font size, H1/H2 gain GitHub-style rules, tables get striped rows, blockquotes gain a quiet fill, code-block headers are calmer, and list markers use the muted color while task checkboxes keep the accent _(PR [#18](https://github.com/sunny0826/mdbijou/pull/18) by [@sunny0826](https://github.com/sunny0826))_
- macOS: make the warm paper `bijou-light` theme the default and follow the system appearance between the bijou light/dark pair; share border, radius, and modal-overlay values as design tokens so panels and dialogs stay consistent _(PR [#17](https://github.com/sunny0826/mdbijou/pull/17) by [@sunny0826](https://github.com/sunny0826))_
- macOS: preview headings use an editorial serif display face (New York with Songti/PingFang fallback) with a stronger size hierarchy, and the default body line spacing is now 1.6 _(PR [#17](https://github.com/sunny0826/mdbijou/pull/17) by [@sunny0826](https://github.com/sunny0826))_
- macOS: edit/preview is now a segmented control; the toolbar gains a hairline divider; the status bar shows a mode indicator and the theme name in monospace; the table-of-contents highlights the active entry with an accent bar _(PR [#17](https://github.com/sunny0826/mdbijou/pull/17) by [@sunny0826](https://github.com/sunny0826))_
- macOS: MDX cards and step badges move to the muted pastel palette, task lists render as drawn checkboxes instead of text glyphs, and preview images get rounded corners with hairline borders _(PR [#17](https://github.com/sunny0826/mdbijou/pull/17) by [@sunny0826](https://github.com/sunny0826))_
- macOS: migrate the native application shell, editor, settings window, toolbar, scrolling preview, and document rendering to GPUI while retaining Markdown, HTML, MDX, Mermaid, theme, and accessibility behavior _(PR [#16](https://github.com/sunny0826/mdbijou/pull/16) by [@sunny0826](https://github.com/sunny0826))_

### Fixed

- macOS: all keyboard shortcuts (⌘O/⌘S/⌘T/⌘E/⌘,/⌘R) stopped responding in the default preview mode after the GPUI migration because the dispatch focus path was empty; the main view now anchors a focus handle so bindings dispatch again _(PR [#18](https://github.com/sunny0826/mdbijou/pull/18) by [@sunny0826](https://github.com/sunny0826))_
- macOS: clicking Open/Save crashed the app with `RefCell already borrowed` because the sync file dialog blocked inside the click handler; file dialogs are now async sheets whose results apply on the next render pass _(PR [#18](https://github.com/sunny0826/mdbijou/pull/18) by [@sunny0826](https://github.com/sunny0826))_
- macOS: task-list checkboxes never rendered — blank lines between items hid the `[x]`/`[ ]` markers from the parser; markers are now found and mixed lists render plain bullets for unmarked items _(PR [#18](https://github.com/sunny0826/mdbijou/pull/18) by [@sunny0826](https://github.com/sunny0826))_
- macOS: list bullets and numbers drifted to the vertical middle of multi-line items; markers now align with the first line and checkboxes center on the first text line _(PR [#18](https://github.com/sunny0826/mdbijou/pull/18) by [@sunny0826](https://github.com/sunny0826))_
- macOS: long list and table content overflowed the window instead of wrapping (flex `min-width: auto`) _(PR [#18](https://github.com/sunny0826/mdbijou/pull/18) by [@sunny0826](https://github.com/sunny0826))_
- macOS: restore responsive preview margins, directory navigation, icons, Mermaid sizing, and reliable remote-image loading when a stale localhost proxy is configured _(PR [#16](https://github.com/sunny0826/mdbijou/pull/16) by [@sunny0826](https://github.com/sunny0826))_

## v0.0.3

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
