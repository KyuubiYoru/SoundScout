#!/usr/bin/env fish
# Build SoundScout as an AppImage.
#
# - Use project-local CLI + explicit `node` (not `npx`).
# - Do not use a variable named `version`: Fish 4+ reserves it (read-only).
#
# Packaging uses `tauri build --no-bundle` + a minimal AppDir + `appimagetool`.
# This avoids Tauri's bundled `linuxdeploy` step (often fails when PATH/env
# differs from an IDE terminal, e.g. Cursor vs Konsole).
#
# Requires: npm install, Rust toolchain, appimagetool (AppImageKit) on PATH.
#
# Usage: fish scripts/build-appimage.fish

set -l script_dir (path dirname (status filename))
set -l root (path dirname $script_dir)
cd $root || exit 1

set -l product_name SoundScout
set -l tauri_cli $root/node_modules/.bin/tauri

if not test -x $tauri_cli
    echo "error: $tauri_cli missing or not executable — run: npm install" >&2
    exit 1
end

set -l node_exe (command -v node)
if test -z "$node_exe"
    echo "error: node not found on PATH" >&2
    exit 1
end

set -l pkg_version
if command -v jq >/dev/null
    set pkg_version (jq -r .version $root/package.json)
else
    set pkg_version (grep -m1 '"version"' $root/package.json | sed -E 's/.*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')
end
if test -z "$pkg_version" || test "$pkg_version" = "null"
    echo "error: could not read version from package.json" >&2
    exit 1
end

set -l bundle_dir $root/src-tauri/target/release/bundle/appimage
set -l appdir "$bundle_dir/$product_name.AppDir"
set -l out_image "$bundle_dir/$product_name"_"$pkg_version"_amd64.AppImage

set -l repair_script $root/scripts/repair-appdir-for-appimage.sh
set -l assemble_script $root/scripts/assemble-appdir-for-appimage.sh

if not test -f $repair_script
    echo "error: missing $repair_script" >&2
    exit 1
end
if not test -f $assemble_script
    echo "error: missing $assemble_script" >&2
    exit 1
end

if not command -v appimagetool >/dev/null
    echo "error: appimagetool not found on PATH (install AppImageKit / appimagetool package)" >&2
    exit 1
end

echo "==> tauri build --no-bundle (compile app + frontend; skip linuxdeploy) — $node_exe $tauri_cli"
$node_exe $tauri_cli build --no-bundle --ci || exit $status

echo "==> assemble AppDir (no linuxdeploy) — $assemble_script"
bash $assemble_script $root $product_name $pkg_version || exit $status

if not test -d $appdir
    echo "error: AppDir not found at $appdir" >&2
    exit 1
end

echo "==> repair AppDir symlinks for AppImageLauncher / spec"
bash $repair_script $appdir $product_name || exit $status

echo "==> appimagetool → $out_image"
appimagetool $appdir $out_image || exit $status

echo "==> done: $out_image"
