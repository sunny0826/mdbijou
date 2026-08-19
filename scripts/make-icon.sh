#!/usr/bin/env bash
# Generates assets/mdbijou.icns from the macOS-style master icon.
#
# Pipeline:
#   1. icon-compose.py (Pillow) scales the full-bleed logo.png artwork to
#      assets/mdbijou-icon-1024.png. Skipped silently if python3/Pillow
#      is unavailable (falls back to using logo.png directly).
#   2. sips + iconutil turn the master into the .icns iconset.
#
# Run:  ./scripts/make-icon.sh
set -euo pipefail

OUT_DIR="assets"
MASTER="$OUT_DIR/mdbijou-icon-1024.png"
SRC="logo.png"
ICONSET="$OUT_DIR/mdbijou.iconset"
ICNS="$OUT_DIR/mdbijou.icns"

# Step 1: compose the macOS-style master icon (best effort).
if python3 -c "import PIL" 2>/dev/null; then
    python3 "$(dirname "$0")/icon-compose.py" "$SRC" "$MASTER"
else
    echo "note: Pillow not found, using $SRC as-is (flat icon)" >&2
    MASTER="$SRC"
fi

[[ -f "$MASTER" ]] || { echo "error: '$MASTER' not found" >&2; exit 1; }

# Step 2: iconset -> icns.
mkdir -p "$ICONSET"
gen() { # gen <name> <px>
    sips -z "$2" "$2" "$MASTER" --out "$ICONSET/$1" >/dev/null
}

gen icon_16x16.png       16
gen icon_16x16@2x.png    32
gen icon_32x32.png       32
gen icon_32x32@2x.png    64
gen icon_128x128.png     128
gen icon_128x128@2x.png  256
gen icon_256x256.png     256
gen icon_256x256@2x.png  512
gen icon_512x512.png     512
gen icon_512x512@2x.png  1024

iconutil -c icns "$ICONSET" -o "$ICNS"
rm -rf "$ICONSET"
echo "created $ICNS"
