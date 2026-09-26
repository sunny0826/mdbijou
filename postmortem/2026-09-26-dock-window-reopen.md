# Postmortem: Closing the document window leaves no way back from the Dock

Date: 2026-09-26

## What happened

After the GPUI migration, closing the document window left mdbijou running
without a document window. Clicking its Dock icon did not open a replacement.
Finder file-open requests also depended on the closed document view.

## Root cause

The runtime created its only document window inside the one-shot application
launch callback and never registered GPUI's `Application::on_reopen` handler.
macOS keeps the process running after its last window closes, so clicking the
Dock icon does not execute the launch callback again.

The `on_open_urls` callback sent paths to a receiver owned by the document view.
Closing the view dropped that receiver; if Settings retained the view, there
was still no document window rendering and consuming the paths. Even with a
window present, receiving a path did not itself schedule a render.

Native verification also exposed an incorrect unsaved indicator after file
opening: setting the editor's text emits a deferred `InputEvent::Change`, which
unconditionally marked the already-loaded document dirty.

## Fix applied

- Register Dock reopen and Finder file-open callbacks before launching the UI.
- Queue both kinds of request in an application-owned async channel that wakes
  the foreground executor and survives the lifetime of individual windows.
- Share document-window creation between startup and subsequent requests,
  tracking its handle separately from Settings windows. Reuse a live document
  window; recreate a closed one as an empty document using saved preferences.
- Route file requests through the existing unsaved-changes guard rather than
  replacing edited documents directly.
- Ignore editor change notifications whose text already matches the document
  source, keeping loaded documents clean while still detecting real edits.

## What we learned

Window lifetime and process lifetime are separate on macOS. Native callbacks
and their receivers must belong to the process when they can arrive without
any document window. Lifecycle regression tests cover repeated close/reopen,
duplicate reopen requests, launch-time file requests, Finder requests with no
document window (including a remaining Settings window), and unsaved changes.
GPUI 0.2.2's test platform does not implement native reopen events, so these
tests exercise the shared request queue; native Dock behavior also needs a
packaged-app smoke test.

## Validation

- `cargo test`: 86 passed, 3 existing network-dependent tests ignored.
- Added clean-document assertions that fail before the editor-event fix, plus
  a regression test that changed editor text is still marked unsaved.
- `just clippy` and `just check`: passed.
- Formatting of the changed Rust files and `git diff --check`: passed.
- Reproduced the old bundled application's failure after closing an empty
  window: the process remained alive and reopening the application in Finder
  did not restore a window.
- Rebuilt and signed `dist/mdbijou.app` and launched the updated application.
  Finder reopening restored an empty window without restarting the
  application. Opening `sample.md` from Finder after another close
  restored the preview with a clean document title in the same process.
  Finder reopening exercises the same macOS reopen callback as a Dock click;
  direct Dock clicking was not automated because its accessibility surface
  was unavailable.
- `just baseline`: release executable 17,777,024 bytes; all five startup
  probes alive in 15–22 ms (process-alive checks, not first-frame timings).
