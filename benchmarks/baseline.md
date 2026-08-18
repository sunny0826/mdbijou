# mdbijou baseline (M0 · T-006)
Date: 2026-08-18
Host: macOS (Apple Silicon)

## Release binary
- Size: 5.3 MB (5,565,056 bytes)  — target ≤10 MB (ideal <5 MB) ✅
- Profile: opt-level="z", lto="fat", codegen-units=1, panic="abort", strip="symbols"

## Startup
- Method: spawn `mdbijou sample.md`, poll first-alive (5ms tick), 3 runs
- Result: alive on first poll in all runs → ~instant, <150 ms target ✅
- (hyperfine not installed; /usr/bin/time measures until window-close, so a
   spawn-to-first-alive proxy is used instead.)

## CLI (text, deterministic)
- `--version`, `--help`, `--list-themes` exit cleanly with correct output.

## Smoke test
- Preview mode (`mdbijou sample.md`): window opens, CJK + code block + table render, no panic.
- Edit mode (`mdbijou --edit sample.md`): syntax-highlighted TextEdit opens, no panic.
