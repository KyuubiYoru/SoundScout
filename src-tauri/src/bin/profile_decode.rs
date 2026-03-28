//! CLI: profile Symphonia decode + IPC-sized copy for a WAV (or other) path.
//!
//! ```text
//! cargo run -p tauri-app --bin profile_decode -- "/path/to/file.wav"
//! ```
//!
//! If no argument is given, tries the GDC sample path relative to the repo root
//! (only succeeds if you have copied that file into `Test Audio Files/...`).

use std::path::PathBuf;

use tauri_app_lib::audio::profile::decode_path_report;

fn default_sample_path() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rel = manifest_dir.join(
        "../Test Audio Files/Sonniss.com - GDC 2019 - Game Audio Bundle Part 1of8/344 Audio - Low Frequency Elements/Alien Spaceship Filtered, Rumble.wav",
    );
    rel.canonicalize().ok()
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var("SOUNDSCOUT_WAV").ok().map(PathBuf::from))
        .or_else(default_sample_path)
        .unwrap_or_else(|| {
            eprintln!(
                "Usage: profile_decode <path-to-audio>\n\
                 Or set SOUNDSCOUT_WAV, or place the GDC sample WAV under Test Audio Files/..."
            );
            std::process::exit(2);
        });

    if !path.exists() {
        eprintln!("File not found: {}", path.display());
        std::process::exit(1);
    }

    match decode_path_report(&path) {
        Ok(r) => {
            println!("{}", serde_json::to_string_pretty(&r).expect("json"));
            eprintln!(
                "\nInterpretation:\n\
                 - decode_wall_ms: Symphonia full-file decode (PCM fallback path).\n\
                 - f32_to_bytes_vec_ms: copying decoded f32 PCM into a Vec<u8> (IPC payload shape).\n\
                 - If these are low but the app is still slow, the bottleneck is likely the WebView\n\
                   `<audio src=asset://…>` pipeline or virtualized/cloud-sync I/O, not Rust decode.\n\
                 - sniff_error set => non-standard RIFF (e.g. RF64); Symphonia may still decode.\n"
            );
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
