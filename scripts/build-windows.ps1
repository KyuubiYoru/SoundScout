#Requires -Version 5.1
<#
.SYNOPSIS
  Build SoundScout for Windows (x86_64-pc-windows-msvc).

  Uses project-local Tauri CLI via node + @tauri-apps/cli/tauri.js (same idea as build-appimage.fish).

  Prerequisites:
  - npm install
  - Rust: rustup target add x86_64-pc-windows-msvc
  - Visual Studio Build Tools (MSVC) or VS with C++ workload

  Usage (from repo root, in PowerShell):
    .\scripts\build-windows.ps1
#>

$windowsTarget = 'x86_64-pc-windows-msvc'

$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root

$tauriJs = Join-Path $root 'node_modules\@tauri-apps\cli\tauri.js'
if (-not (Test-Path -LiteralPath $tauriJs)) {
    Write-Error "error: $tauriJs missing — run: npm install"
    exit 1
}

$node = Get-Command node -ErrorAction SilentlyContinue
if (-not $node) {
    Write-Error 'error: node not found on PATH'
    exit 1
}

if (Get-Command rustup -ErrorAction SilentlyContinue) {
    $hasTarget = $false
    rustup target list --installed | ForEach-Object {
        if ($_ -eq $windowsTarget) {
            $hasTarget = $true
        }
    }
    if (-not $hasTarget) {
        Write-Error "error: Rust target $windowsTarget not installed — run: rustup target add $windowsTarget"
        exit 1
    }
} else {
    Write-Warning "rustup not on PATH; skipping target check (ensure $windowsTarget is available)"
}

$outRoot = Join-Path $root "src-tauri\target\$windowsTarget\release\bundle"

Write-Host "==> tauri build --target $windowsTarget — $($node.Path) $tauriJs"
& $node.Path $tauriJs build --target $windowsTarget --ci
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "==> done: Windows bundles under $outRoot"
