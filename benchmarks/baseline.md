# mdbijou baseline (v0.0.1)
Date: 2026-08-19
Host: macOS (Apple Silicon)

## Release binary
- Size: 7.2 MB (7,536,304 bytes)  — target ≤10 MB (ideal <5 MB) ✅
- Profile: opt-level="z", lto="fat", codegen-units=1, panic="abort", strip="symbols"

## Startup
- Method: spawn `mdbijou sample.md`, poll first-alive (2ms tick), 5 runs
- Result: 16–22 ms spawn-to-first-alive (avg ~18 ms), <150 ms target ✅
- (hyperfine not installed; /usr/bin/time measures until window-close, so a
   spawn-to-first-alive proxy is used instead.)

## CLI (text, deterministic)
- `--version`, `--help`, `--list-themes` exit cleanly with correct output.

## Smoke test
- Preview mode (`mdbijou sample.md`): window opens, CJK + code block + table render, no panic.
- Edit mode (`mdbijou --edit sample.md`): syntax-highlighted TextEdit opens, no panic.
