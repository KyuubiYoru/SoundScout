#!/usr/bin/env fish
# Legacy: SoundVault uses built-in ONNX embeddings (fastembed) by default — no Python required.
# This script only helps if you experiment with python/embed_sidecar.py manually.
#
# Usage:  fish scripts/embed-env.fish
#         fish scripts/embed-env.fish --install-only   # no new shell

set -l this_script (status filename)
if test -z "$this_script"
    echo "embed-env: could not resolve script path; run: fish scripts/embed-env.fish (do not source)" >&2
    exit 1
end
set -l script_dir (dirname "$this_script")
set -l repo_root (cd "$script_dir/.." && pwd) || exit 1
set -l venv $repo_root/.venv-embed
set -l req $repo_root/python/requirements.txt

if not test -f "$req"
    echo "embed-env: missing $req" >&2
    exit 1
end

if not test -d "$venv"
    echo "embed-env: creating venv at $venv"
    python3 -m venv "$venv" || exit 1
end

set -l pip "$venv/bin/pip"
set -l py "$venv/bin/python"

if not test -x "$pip"
    echo "embed-env: expected $pip" >&2
    exit 1
end

echo "embed-env: installing (upgrade pip, then requirements)…"
"$pip" install -q --upgrade pip
"$pip" install -r "$req" || exit 1

echo "embed-env: ok — interpreter is $py"

set -l install_only 0
for a in $argv
    if test "$a" = --install-only
        set install_only 1
    end
end

echo ""
echo "Point SoundVault at this Python (required for Tauri rebuild):"
echo "  set -gx SOUNDVAULT_EMBED_PYTHON $py"
echo ""

if test $install_only -eq 1
    exit 0
end

echo "Spawning a new fish with SOUNDVAULT_EMBED_PYTHON set (exit to leave)."
exec fish -C "set -gx SOUNDVAULT_EMBED_PYTHON '$py'"
