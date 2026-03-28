#!/usr/bin/env bash
# AppImageLauncher (and the AppImage spec) expect root-level .desktop and .DirIcon
# symlinks with targets relative to the AppDir. Tauri currently creates absolute
# symlinks, which break once the image is mounted elsewhere.
set -euo pipefail
APPDIR="${1:?Usage: $0 <path/to/ProductName.AppDir> [productName]}"
PRODUCT="${2:-SoundScout}"
cd "$APPDIR"
rm -f "${PRODUCT}.desktop" .DirIcon
ln -s "usr/share/applications/${PRODUCT}.desktop" "${PRODUCT}.desktop"
ln -s "${PRODUCT}.png" .DirIcon
