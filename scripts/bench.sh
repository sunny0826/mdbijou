#!/usr/bin/env bash
# mdbijou baseline measurement: release size + startup (spawn-to-first-alive).
set -euo pipefail

BIN="${1:-target/release/mdbijou}"
SAMPLE="${2:-sample.md}"

if [[ ! -x "$BIN" ]]; then
    echo "release binary not found — building first..." >&2
    cargo build --release
fi

echo "== size =="
SIZE=$(stat -f%z "$BIN" 2>/dev/null || stat -c%z "$BIN")
echo "release binary: $(ls -lh "$BIN" | awk '{print $5}')  ($SIZE bytes)"

echo ""
echo "== startup (spawn-to-first-alive, 5 runs) =="
for i in 1 2 3 4 5; do
    START=$(python3 -c 'import time;print(time.time())')
    "$BIN" "$SAMPLE" >/dev/null 2>&1 &
    PID=$!
    ALIVE=0
    for _ in $(seq 1 1000); do
        if kill -0 "$PID" 2>/dev/null; then ALIVE=1; break; fi
        sleep 0.002
    done
    END=$(python3 -c 'import time;print(time.time())')
    MS=$(python3 -c "print(f'{($END-$START)*1000:.0f}')")
    echo "  run $i: alive=$ALIVE  ~${MS}ms"
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
done

echo ""
echo "== CLI sanity =="
"$BIN" --version
