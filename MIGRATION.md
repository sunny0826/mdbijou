# GPUI migration contract

## Scope

- Keep the current Markdown/MDX `Document` IR as the source of truth.
- GPUI is the sole application frontend; the former egui shell and direct
  dependencies have been removed after the cutover checks passed.
- Preserve the Paper & Jewel visual language while improving native motion and
  interaction quality.

## Explicit deferral

`gpui_component::text::TextView::markdown` is not part of this migration.
Evaluate it only after the GPUI application has replaced egui and passed the
full functional, visual, accessibility, performance, and packaging checks.

## Migration gates

- Existing document/parser tests remain green.
- GPUI's code-editor input provides CJK IME, keyboard navigation, selection,
  and undo/redo; save flows and macOS file opening are wired through the GPUI
  application state.
- Release size stays within 30% of the 17,083,104-byte migration baseline.
- Startup stays below 150 ms using `just baseline`.
