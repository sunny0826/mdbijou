#!/usr/bin/env bash
# Packages mdbijou as a macOS .app bundle (and optionally a .dmg).
#
# Usage:
#   ./scripts/bundle.sh           # build release + create dist/mdbijou.app
#   ./scripts/bundle.sh --dmg     # also create dist/mdbijou-<version>.dmg
#   ./scripts/bundle.sh --skip-build
set -euo pipefail

APP_NAME="mdbijou"
BIN="target/release/$APP_NAME"
DIST="dist"
APP_DIR="$DIST/$APP_NAME.app"
ICNS="assets/mdbijou.icns"

DMG=0
SKIP_BUILD=0
for arg in "$@"; do
    case "$arg" in
        --dmg) DMG=1 ;;
        --skip-build) SKIP_BUILD=1 ;;
        *) echo "unknown option: $arg" >&2; exit 1 ;;
    esac
done

VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -n1)"

if [[ "$SKIP_BUILD" -eq 0 ]]; then
    echo "→ building release binary"
    cargo build --release
fi
[[ -x "$BIN" ]] || { echo "error: '$BIN' not found" >&2; exit 1; }

if [[ ! -f "$ICNS" ]]; then
    echo "→ generating app icon"
    ./scripts/make-icon.sh
fi

echo "→ packaging $APP_DIR (v$VERSION)"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
cp "$BIN" "$APP_DIR/Contents/MacOS/$APP_NAME"
cp "$ICNS" "$APP_DIR/Contents/Resources/mdbijou.icns"

cat > "$APP_DIR/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>mdbijou</string>
    <key>CFBundleDisplayName</key>
    <string>mdbijou</string>
    <key>CFBundleIdentifier</key>
    <string>com.mdbijou.app</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleExecutable</key>
    <string>mdbijou</string>
    <key>CFBundleIconFile</key>
    <string>mdbijou.icns</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key>
            <string>Markdown Document</string>
            <key>CFBundleTypeRole</key>
            <string>Viewer</string>
            <key>LSItemContentTypes</key>
            <array>
                <string>net.daringfireball.markdown</string>
                <string>public.markdown</string>
            </array>
            <key>LSHandlerRank</key>
            <string>Alternate</string>
        </dict>
    </array>
    <key>UTImportedTypeDeclarations</key>
    <array>
        <dict>
            <key>UTTypeIdentifier</key>
            <string>public.markdown</string>
            <key>UTTypeDescription</key>
            <string>Markdown Document</string>
            <key>UTTypeConformsTo</key>
            <array>
                <string>public.plain-text</string>
            </array>
            <key>UTTypeTagSpecification</key>
            <dict>
                <key>public.filename-extension</key>
                <array>
                    <string>md</string>
                    <string>markdown</string>
                </array>
            </dict>
        </dict>
    </array>
</dict>
</plist>
EOF

# Ad-hoc sign so Gatekeeper/LaunchServices treat the bundle consistently.
codesign --force --deep --sign - "$APP_DIR" 2>/dev/null || true

echo "created $APP_DIR"

if [[ "$DMG" -eq 1 ]]; then
    DMG_PATH="$DIST/$APP_NAME-$VERSION.dmg"
    echo "→ creating $DMG_PATH"
    rm -f "$DMG_PATH"
    hdiutil create -volname "$APP_NAME" -srcfolder "$APP_DIR" \
        -ov -format UDZO "$DMG_PATH" >/dev/null
    echo "created $DMG_PATH"
fi
