# SoundScout

A desktop app for browsing and searching large sound effect libraries. Built with Tauri, SvelteKit, and Rust.

## Features

- Scan local folders and index audio files into a searchable library
- Lexical and semantic (embedding-based) search
- Waveform preview with clip selection
- Tagging, ratings, and favorites
- Post-processing: loop detection, normalize, crossfade, trim
- Copy tracks or clips directly to clipboard

## Platform

Tested on Arch-based Linux. A Windows release build is possible but **untested** (build may fail or the app may misbehave until someone validates it).

### Windows build (untested)

Prerequisites: Node.js, Rust, [Tauri's Windows prerequisites](https://tauri.app/start/prerequisites/) (Visual Studio Build Tools with the C++ workload, or full Visual Studio), and the MSVC target:

```powershell
rustup target add x86_64-pc-windows-msvc
```

From the repo root, after `npm install`:

```powershell
.\scripts\build-windows.ps1
```

The script checks prerequisites (Rust target, Node, Tauri CLI) before building. Alternatively, skip the preflight checks with:

```powershell
npm run release:windows
```

Installers and bundles are written under `src-tauri\target\x86_64-pc-windows-msvc\release\bundle\`.

### Linux release build

```bash
npm run release:linux
```

Output is written under `src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/`.

## Dev Setup

Requires Node.js and Rust. The Tauri CLI is installed locally via `npm install` — no global `cargo install tauri-cli` needed.

```bash
npm install
npm run tauri dev
```

## Testing

Frontend (Vitest):

```bash
npm test
```

Backend (Rust):

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```
