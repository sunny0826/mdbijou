#!/usr/bin/env bash
# mdbijou installer — copies the release binary to ~/.local/bin (or /usr/local/bin)
# as the `mdb` CLI.
set -euo pipefail

BIN="${1:-target/release/mdbijou}"
if [[ ! -x "$BIN" ]]; then
    echo "error: '$BIN' not found. Build it first:  cargo build --release" >&2
    exit 1
fi

for DEST in "$HOME/.local/bin" /usr/local/bin; do
    if [[ ":$PATH:" == *":$DEST:"* ]]; then
        TARGET="$DEST"
        break
    fi
    TARGET="${TARGET:-}"
done
TARGET="${TARGET:-$HOME/.local/bin}"

mkdir -p "$TARGET"
cp "$BIN" "$TARGET/mdb"
chmod +x "$TARGET/mdb"
echo "installed mdb -> $TARGET/mdb"
echo "ensure '$TARGET' is on your PATH, then run:  mdb path/to/file.md"
