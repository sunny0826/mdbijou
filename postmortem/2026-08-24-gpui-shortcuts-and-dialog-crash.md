# Postmortem: Keyboard shortcuts dead in preview mode + Open/Save crash (GPUI migration follow-up)

Date: 2026-08-24
PR: (this PR)

## What happened

After the GPUI migration (#16) and the editorial UI polish (#17), three
user-visible regressions shipped together:

1. **Every keyboard shortcut stopped responding** (⌘O/⌘S/⌘T/⌘E/⌘,/⌘R) while
   the app was in the default preview mode. Mouse interaction kept working,
   which made the app look "half alive".
2. **Clicking the Open or Save toolbar button crashed the app** with
   `RefCell already borrowed` inside `gpui::app::AppCell::borrow_mut`.
3. **Task-list checkboxes never rendered** — `- [x]` items drew as plain
   bullets — and list bullets drifted to the vertical middle of multi-line
   items; long list/table content overflowed the window instead of wrapping.

## Root causes

1. **Empty focus path.** GPUI dispatches bound actions along the window's
   focus path. `Window::focus_path()` returns an empty vector when no element
   has focus. Preview mode contains no focusable element at all, so the path
   was always empty and every `KeyBinding` silently no-op'd. Mouse events use
   a separate hit-testing channel, which is why buttons still worked.
2. **Nested modal loop inside a GPUI update borrow.** The Open/Save buttons
   called `rfd::FileDialog::pick_file()` synchronously inside the click
   listener. The dialog blocks the main thread in a nested run loop; while it
   was open, macOS delivered a keyboard-layout-change event and GPUI tried to
   re-enter `AppCell::borrow_mut()` — already borrowed by the click handler —
   and panicked.
3. **Loose-list parsing.** With a blank line between list items,
   pulldown-cmark wraps item content in a `Paragraph` frame. The
   `TaskListMarker` handler only inspected the top of the parse stack, found
   the paragraph instead of the enclosing item, and dropped the marker.
4. **Layout details.** Flex items default to `min-width: auto`, so long text
   pushed rows past the window edge instead of wrapping, and the list-marker
   column centered markers across the whole (possibly multi-line) item.

## Fix applied

1. The main view owns a `FocusHandle`, the content div `track_focus`s it, and
   focus is seeded at startup and re-seated whenever the view toggles.
   Regression-tested with `#[gpui::test]` + `simulate_keystrokes("cmd-t")`
   (negative-verified: removing the fix turns the test red).
2. File dialogs moved to `rfd::AsyncFileDialog` (non-blocking sheet on
   macOS). Results land in `deferred_open` / `deferred_save` and are applied
   on the next render pass where `&mut Window` is available again.
3. `TaskListMarker` now searches the stack for the enclosing `Ctx::Item`, and
   `Block::TaskList::checked` became `Vec<Option<bool>>` so mixed lists
   render plain bullets for unmarked items (GitHub behavior).
4. `min_w_0()` on flex content columns, `items_start()` on the marker column,
   and a first-line offset for checkboxes.

## What we learned

- **GPUI keybindings require a non-empty focus path.** Any window whose
  content can lack focusable elements must anchor a `FocusHandle` and keep it
  focused. This is now covered by an automated keystroke-dispatch test.
- **Never run nested modal loops inside a GPUI update borrow.** Dialogs must
  be async and apply their result after the update unwinds.
- **Process:** the shortcuts regression shipped in #16 and survived #17
  because verification was manual and mouse-only. The new keystroke-dispatch
  tests run in `cargo test`, so future framework work gets an automatic
  signal.
