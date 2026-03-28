#!/usr/bin/env bash
# Build a minimal FHS AppDir from a release binary (no linuxdeploy).
# Args: <repo-root> <productName> <packageVersion>
set -euo pipefail

ROOT="${1:?usage: $0 <repo-root> <productName> <packageVersion>}"
PRODUCT="${2:?}"
PKG_VERSION="${3:?}"

BUNDLE_DIR="$ROOT/src-tauri/target/release/bundle/appimage"
APPDIR="$BUNDLE_DIR/${PRODUCT}.AppDir"
BINARY="$ROOT/src-tauri/target/release/$PRODUCT"
ICONS_DIR="$ROOT/src-tauri/icons"

if [[ ! -x "$BINARY" ]]; then
  echo "error: release binary not found or not executable: $BINARY" >&2
  echo "hint: run tauri build --no-bundle first" >&2
  exit 1
fi

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/share/applications"
mkdir -p "$APPDIR/usr/share/icons/hicolor/32x32/apps"
mkdir -p "$APPDIR/usr/share/icons/hicolor/128x128/apps"
mkdir -p "$APPDIR/usr/share/icons/hicolor/256x256/apps"

cp "$BINARY" "$APPDIR/usr/bin/$PRODUCT"
chmod +x "$APPDIR/usr/bin/$PRODUCT"

cp "$ICONS_DIR/32x32.png" "$APPDIR/usr/share/icons/hicolor/32x32/apps/${PRODUCT}.png"
cp "$ICONS_DIR/128x128.png" "$APPDIR/usr/share/icons/hicolor/128x128/apps/${PRODUCT}.png"
cp "$ICONS_DIR/128x128@2x.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/${PRODUCT}.png"

cat >"$APPDIR/usr/share/applications/${PRODUCT}.desktop" <<EOF
[Desktop Entry]
Name=$PRODUCT
Comment=SoundScout $PKG_VERSION — search and export game audio SFX
Exec=$PRODUCT
Icon=$PRODUCT
Type=Application
Categories=AudioVideo;Audio;
EOF

# AppRun: required entry point for AppImage type 2
printf '%s\n' \
  '#!/bin/sh' \
  'HERE="$(dirname "$(readlink -f "${0}")")"' \
  "exec \"\$HERE/usr/bin/$PRODUCT\" \"\$@\"" >"$APPDIR/AppRun"
chmod +x "$APPDIR/AppRun"

# Root icon for .DirIcon symlink (repair script expects PRODUCT.png next to AppRun)
cp "$ICONS_DIR/128x128.png" "$APPDIR/${PRODUCT}.png"
