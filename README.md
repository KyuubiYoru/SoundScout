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

Only tested on Arch-based Linux. Windows support is planned but not yet available.

## Dev Setup

Requires Node.js, Rust, and the Tauri CLI prerequisites for your platform.

```bash
npm install
npm run tauri dev
```
