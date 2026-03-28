# soundvault-loop — Rust Library Implementation Plan

## Overview

A standalone Rust library crate that provides offline audio loop analysis and processing. Soundvault calls into this library to analyze imports, score loopability, find optimal loop points, and produce seamlessly looping output files. The library is engine-agnostic — it operates on sample buffers and WAV files, with no runtime audio playback dependency.

---

## Crate Architecture

```
soundvault-loop/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # Public API surface
│   ├── analysis/
│   │   ├── mod.rs
│   │   ├── classify.rs         # Sound type classification (periodic/aperiodic/quasi)
│   │   ├── autocorrelation.rs  # FFT-accelerated autocorrelation + AMDF
│   │   ├── spectral.rs         # STFT features: chroma, MFCC, centroid
│   │   ├── zero_crossing.rs    # Zero-crossing detection + slope matching
│   │   ├── envelope.rs         # Envelope follower, attack/sustain/release detection
│   │   └── loop_finder.rs      # Composite loop-point scoring + candidate ranking
│   ├── processing/
│   │   ├── mod.rs
│   │   ├── crossfade.rs        # Linear, equal-power, correlation-adaptive curves
│   │   ├── split_swap.rs       # Split-swap-crossfade (ambient technique)
│   │   ├── spectral_blend.rs   # STFT-domain crossfade for tonal material
│   │   ├── normalize.rs        # Peak normalization, RMS matching
│   │   ├── trim.rs             # Silence trimming, attack isolation
│   │   └── resample.rs         # Sample rate conversion wrapper
│   ├── wav/
│   │   ├── mod.rs
│   │   ├── reader.rs           # Thin wrapper over hound with metadata extraction
│   │   ├── writer.rs           # WAV writing with smpl chunk support
│   │   └── smpl.rs             # smpl chunk parser/writer (loop points, sample metadata)
│   ├── types.rs                # Core types: AudioBuffer, LoopPoint, LoopCandidate, etc.
│   └── error.rs                # Error types
└── tests/
    ├── integration/
    │   ├── ambient_loop.rs
    │   ├── tonal_loop.rs
    │   └── music_loop.rs
    └── fixtures/                # Small WAV test files
```

---

## Core Types

```rust
// types.rs

/// Interleaved f32 sample buffer — the universal internal format
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Classification of a sound's periodicity
pub enum SoundClass {
    /// Strong autocorrelation peak (r > 0.7) — beam weapons, drones, hums
    Periodic { fundamental_hz: f32, confidence: f32 },
    /// Moderate autocorrelation — engines, machinery with variation
    QuasiPeriodic { approx_hz: f32, confidence: f32 },
    /// No dominant period — wind, fire, noise textures
    Aperiodic,
}

/// A candidate loop region within an audio buffer
pub struct LoopCandidate {
    /// Sample index where the loop body starts (after attack)
    pub start: usize,
    /// Sample index where the loop body ends (before release)
    pub end: usize,
    /// Composite quality score 0.0–1.0
    pub score: f32,
    /// Breakdown of individual metrics
    pub metrics: LoopMetrics,
}

pub struct LoopMetrics {
    /// Normalized cross-correlation at loop boundary
    pub correlation: f32,
    /// AMDF score (lower = better match)
    pub amdf_score: f32,
    /// Spectral similarity (cosine distance of MFCC vectors)
    pub spectral_similarity: f32,
    /// Zero-crossing alignment quality
    pub zero_crossing_quality: f32,
    /// Slope (derivative) match at boundary
    pub slope_match: f32,
}

/// Describes how to crossfade at loop boundaries
pub enum CrossfadeMode {
    /// Automatically select based on correlation measurement
    Auto,
    /// Linear ramp — good when r ≈ 1.0
    Linear,
    /// sin/cos constant-power — good when r ≈ 0.0
    EqualPower,
    /// Bristow-Johnson adaptive curve — optimal for any r
    CorrelationAdaptive,
}

/// Full configuration for the loop processor
pub struct LoopConfig {
    /// Minimum loop duration in seconds
    pub min_loop_sec: f32,
    /// Maximum loop duration in seconds
    pub max_loop_sec: f32,
    /// Crossfade duration in seconds (or None for auto)
    pub crossfade_sec: Option<f32>,
    /// Crossfade curve selection
    pub crossfade_mode: CrossfadeMode,
    /// Number of top candidates to return
    pub max_candidates: usize,
    /// Whether to embed smpl chunk metadata in output
    pub embed_loop_metadata: bool,
}

/// Result of loop analysis — returned to soundvault for display/selection
pub struct LoopAnalysis {
    pub sound_class: SoundClass,
    pub candidates: Vec<LoopCandidate>,
    /// Detected attack end (sample index)
    pub attack_end: Option<usize>,
    /// Detected release start (sample index)
    pub release_start: Option<usize>,
}

/// Result of loop processing
pub struct LoopResult {
    pub buffer: AudioBuffer,
    pub loop_start: usize,
    pub loop_end: usize,
    /// Which technique was applied
    pub technique: LoopTechnique,
}

pub enum LoopTechnique {
    PitchSynchronous { periods: u32 },
    SplitSwapCrossfade { crossfade_samples: usize },
    SpectralBlend { fft_size: usize, crossfade_frames: usize },
    TailToFrontCrossfade { crossfade_samples: usize },
}
```

---

## Public API Surface

```rust
// lib.rs — what soundvault calls

pub use types::*;
pub use error::LoopError;

/// Analyze an audio file and return loop candidates without modifying it.
/// This is the "read-only" entry point — call it on import to score loopability.
pub fn analyze(buffer: &AudioBuffer, config: &LoopConfig) -> Result<LoopAnalysis, LoopError>;

/// Analyze directly from a file path (convenience wrapper).
pub fn analyze_file(path: &Path, config: &LoopConfig) -> Result<LoopAnalysis, LoopError>;

/// Process an audio buffer into a seamlessly looping version.
/// Uses the best candidate from analysis, or a specific candidate index.
pub fn make_loop(
    buffer: &AudioBuffer,
    config: &LoopConfig,
    candidate: Option<usize>,
) -> Result<LoopResult, LoopError>;

/// Process and write directly to a WAV file with embedded smpl loop metadata.
pub fn make_loop_file(
    input: &Path,
    output: &Path,
    config: &LoopConfig,
    candidate: Option<usize>,
) -> Result<LoopResult, LoopError>;

/// Apply split-swap-crossfade to an audio buffer (ambient/texture shortcut).
/// Does not require analysis — always produces a valid loop.
pub fn split_swap(
    buffer: &AudioBuffer,
    crossfade_sec: f32,
) -> Result<LoopResult, LoopError>;

/// Peak normalize a buffer to a target amplitude (0.0–1.0).
pub fn normalize_peak(buffer: &mut AudioBuffer, target: f32);

/// Trim leading/trailing silence below a threshold (in dB).
pub fn trim_silence(buffer: &mut AudioBuffer, threshold_db: f32, min_silence_ms: f32);

/// Read a WAV file into an AudioBuffer, including any smpl chunk loop points.
pub fn read_wav(path: &Path) -> Result<(AudioBuffer, Option<Vec<SmplLoopPoint>>), LoopError>;

/// Write an AudioBuffer to WAV with optional smpl chunk loop points.
pub fn write_wav(
    path: &Path,
    buffer: &AudioBuffer,
    loops: Option<&[SmplLoopPoint]>,
) -> Result<(), LoopError>;
```

---

## Dependencies

```toml
[package]
name = "soundvault-loop"
version = "0.1.0"
edition = "2021"

[dependencies]
# WAV I/O — read/write PCM and float WAV files
hound = "3.5"

# FFT — real-to-complex, ~2x faster than complex FFT for audio
realfft = "3"

# Sample type conversions, interpolation primitives
dasp_sample = "0.11"

# Resampling (optional, for sample rate normalization)
rubato = "0.15"

# Error handling
thiserror = "2"

# Logging
log = "0.4"

[dev-dependencies]
# Test assertions with floating point tolerance
approx = "0.5"
```

**What's NOT a dependency and why:**

- `rodio` / `kira` — playback is soundvault's job, not the library's
- `fundsp` — synthesis is out of scope; this library processes existing audio
- `symphonia` — hound covers WAV which is the pipeline format; soundvault handles format decoding before passing buffers in
- `ebur128` — LUFS normalization is a stretch goal; peak normalization covers the immediate need

---

## Module Implementation Details

### Phase 1 — Foundation (Core types + WAV I/O + basic processing)

**Goal:** Read WAV files, normalize, trim silence, write WAV with smpl chunks. Soundvault can import audio and run basic preprocessing.

#### 1a. `wav/smpl.rs` — WAV smpl chunk parser/writer

The smpl chunk is a well-defined binary structure that hound doesn't parse. Custom implementation needed (~200 lines).

```
smpl chunk layout:
  manufacturer:     u32
  product:          u32
  sample_period:    u32  (nanoseconds per sample = 1e9 / sample_rate)
  midi_unity_note:  u32
  midi_pitch_frac:  u32
  smpte_format:     u32
  smpte_offset:     u32
  num_loops:        u32
  sampler_data_len: u32
  loops[]:
    cue_point_id:   u32
    type:           u32  (0=forward, 1=alternating, 2=reverse)
    start:          u32  (sample offset)
    end:            u32  (sample offset, inclusive)
    fraction:       u32
    play_count:     u32  (0=infinite)
```

Implementation approach:
- Read: after hound finishes decoding, seek to find `smpl` chunk by FourCC scan
- Write: hound doesn't support custom chunks, so write the WAV header + data manually using hound's spec info, then append the smpl chunk before finalizing the RIFF size
- Alternative: write via hound first, then patch the file by appending the smpl chunk and updating the RIFF header size

#### 1b. `processing/normalize.rs`

```rust
/// Find peak amplitude across all channels
fn peak_amplitude(buffer: &AudioBuffer) -> f32;

/// Scale all samples so peak hits target (e.g., 0.97 for -0.26 dBFS)
pub fn normalize_peak(buffer: &mut AudioBuffer, target: f32);

/// Compute RMS of a sample range
fn rms(samples: &[f32]) -> f32;
```

#### 1c. `processing/trim.rs`

```rust
/// Detect leading silence, trailing silence using envelope follower
/// Returns (first_active_sample, last_active_sample)
pub fn detect_silence_bounds(buffer: &AudioBuffer, threshold_db: f32, min_ms: f32) -> (usize, usize);

/// Trim in place
pub fn trim_silence(buffer: &mut AudioBuffer, threshold_db: f32, min_ms: f32);
```

#### 1d. `processing/crossfade.rs`

All crossfade curves as functions that generate gain envelopes:

```rust
/// Generate a crossfade envelope pair (fade_in, fade_out) of length N
pub fn linear(n: usize) -> (Vec<f32>, Vec<f32>);
pub fn equal_power(n: usize) -> (Vec<f32>, Vec<f32>);
pub fn correlation_adaptive(n: usize, r: f32) -> (Vec<f32>, Vec<f32>);

/// Measure normalized cross-correlation between two equal-length slices
pub fn cross_correlation(a: &[f32], b: &[f32]) -> f32;

/// Apply crossfade between tail of buffer and head of buffer (in-place loop seam)
pub fn apply_loop_crossfade(buffer: &mut AudioBuffer, crossfade_samples: usize, mode: CrossfadeMode);
```

**Deliverable:** soundvault can import a WAV, normalize it, trim silence, and re-export with smpl metadata. Foundation for everything else.

---

### Phase 2 — Analysis Engine (Classification + loop-point finding)

**Goal:** Given an audio buffer, classify it and return ranked loop candidates with quality scores.

#### 2a. `analysis/autocorrelation.rs`

FFT-accelerated autocorrelation:

```rust
/// Compute normalized autocorrelation using FFT
/// Returns r[τ] for τ in 0..max_lag
pub fn autocorrelation(samples: &[f32], max_lag: usize) -> Vec<f32>;

/// Find peaks in autocorrelation (candidate periods)
/// Returns Vec<(lag, correlation_value)> sorted by correlation descending
pub fn find_period_peaks(acf: &[f32], min_lag: usize, max_lag: usize) -> Vec<(usize, f32)>;

/// Average Magnitude Difference Function
pub fn amdf(samples: &[f32], max_lag: usize) -> Vec<f32>;

/// Parabolic interpolation for sub-sample peak precision
pub fn refine_peak(values: &[f32], peak_index: usize) -> f32;
```

Implementation: zero-pad to next power of 2 × 2, compute `IFFT(|FFT(x)|²)`, normalize by `R[0]`.

#### 2b. `analysis/classify.rs`

```rust
/// Classify sound based on autocorrelation of its sustained region
pub fn classify(buffer: &AudioBuffer) -> SoundClass;
```

Decision logic:
- Compute autocorrelation over the middle 50% of the buffer (avoids attack/release)
- Find strongest peak at lag > `sample_rate / 2000` (ignore ultra-high freq)
- If peak `r > 0.7` → `Periodic`
- If peak `r > 0.4` → `QuasiPeriodic`
- Else → `Aperiodic`

#### 2c. `analysis/envelope.rs`

```rust
/// Envelope follower with configurable attack/release times
pub fn envelope(samples: &[f32], attack_ms: f32, release_ms: f32, sr: u32) -> Vec<f32>;

/// Detect where the attack transient ends and sustained content begins
/// Returns sample index
pub fn detect_attack_end(buffer: &AudioBuffer) -> Option<usize>;

/// Detect where sustained content ends and release/tail begins
pub fn detect_release_start(buffer: &AudioBuffer) -> Option<usize>;
```

#### 2d. `analysis/zero_crossing.rs`

```rust
/// Find all positive-going zero crossings in a sample range
pub fn positive_zero_crossings(samples: &[f32], start: usize, end: usize) -> Vec<usize>;

/// Score how well two zero-crossing points match (amplitude + slope)
pub fn crossing_match_score(samples: &[f32], pos_a: usize, pos_b: usize) -> f32;
```

#### 2e. `analysis/spectral.rs`

STFT-based feature extraction for spectral similarity scoring:

```rust
/// Compute magnitude spectrum at a given position (single STFT frame)
pub fn magnitude_spectrum(samples: &[f32], center: usize, fft_size: usize, window: &[f32]) -> Vec<f32>;

/// Compute MFCC feature vector (13 coefficients) at a position
pub fn mfcc(samples: &[f32], center: usize, fft_size: usize, sr: u32, n_mfcc: usize) -> Vec<f32>;

/// Compute chroma vector (12 pitch classes) at a position
pub fn chroma(samples: &[f32], center: usize, fft_size: usize, sr: u32) -> [f32; 12];

/// Cosine similarity between two feature vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32;

/// Generate Hanning window of size N
pub fn hanning_window(n: usize) -> Vec<f32>;
```

MFCC implementation: magnitude spectrum → mel filterbank (40 triangular filters) → log energy → DCT → take first 13 coefficients. The mel filterbank and DCT are straightforward to implement without external deps.

#### 2f. `analysis/loop_finder.rs`

The composite scoring engine that ties everything together:

```rust
/// Find and rank loop candidates for the given buffer
pub fn find_loop_candidates(
    buffer: &AudioBuffer,
    class: &SoundClass,
    config: &LoopConfig,
) -> Vec<LoopCandidate>;
```

Strategy per sound class:

**Periodic:** Find fundamental period from autocorrelation → generate candidate loops at 4, 8, 16, 32 periods → for each, refine start/end to positive-going zero crossings → score with cross-correlation + AMDF + spectral similarity → rank.

**QuasiPeriodic:** Sliding window autocorrelation to find the most stable region → extract candidates from that region using the periodic strategy → wider tolerance on scoring.

**Aperiodic:** No period detection. Use envelope analysis to find the most spectrally stationary region → generate candidates of `min_loop_sec` to `max_loop_sec` duration → score on spectral similarity between start and end regions (MFCC + centroid) → these will be processed with split-swap-crossfade rather than phase-aligned looping.

Composite score weighting:

```
score = 0.35 * correlation
      + 0.25 * (1.0 - amdf_normalized)
      + 0.25 * spectral_similarity
      + 0.10 * zero_crossing_quality
      + 0.05 * slope_match
```

**Deliverable:** `analyze()` returns a `LoopAnalysis` with ranked candidates. Soundvault can display loopability scores on import and let the user preview candidates.

---

### Phase 3 — Loop Processing (Producing output files)

**Goal:** Take the best candidate and produce a seamlessly looping WAV with embedded metadata.

#### 3a. `processing/split_swap.rs`

The "bulletproof" ambient technique:

```rust
/// Split audio at midpoint, swap halves, crossfade center seam.
/// Always produces a valid loop regardless of input content.
pub fn split_swap_crossfade(
    buffer: &AudioBuffer,
    crossfade_sec: f32,
) -> Result<LoopResult, LoopError>;
```

Steps:
1. Split buffer at `len / 2`
2. Swap halves: `[B | A]`
3. Equal-power crossfade at the new center seam (where B ends and A begins)
4. Result loops from sample 0 to sample len-1
5. Apply 10-sample micro-fades at file boundaries as safety net

#### 3b. `processing/spectral_blend.rs`

STFT-domain crossfade for tonal/harmonic material:

```rust
/// Blend loop boundary using spectral interpolation.
/// Avoids comb-filtering artifacts from time-domain crossfade of harmonic content.
pub fn spectral_crossfade(
    buffer: &AudioBuffer,
    loop_start: usize,
    loop_end: usize,
    crossfade_frames: usize,
    fft_size: usize,
) -> Result<AudioBuffer, LoopError>;
```

Steps:
1. STFT the tail region (before `loop_end`) and head region (after `loop_start`)
2. For each frame in the crossfade zone, interpolate magnitudes linearly
3. Phase: use weighted circular interpolation (or simpler: carry phase from the outgoing signal and blend toward the incoming)
4. Inverse STFT with overlap-add
5. Replace the boundary region in the buffer

#### 3c. `make_loop` — The orchestrator

```rust
pub fn make_loop(buffer: &AudioBuffer, config: &LoopConfig, candidate: Option<usize>) -> Result<LoopResult, LoopError> {
    let analysis = analyze(buffer, config)?;
    let candidate = match candidate {
        Some(i) => &analysis.candidates[i],
        None => &analysis.candidates[0], // best
    };

    match &analysis.sound_class {
        SoundClass::Periodic { .. } => {
            // Pitch-synchronous: extract loop region, apply correlation-adaptive crossfade
            let r = cross_correlation(/* start region */, /* end region */);
            if r > 0.95 {
                // Minimal crossfade — waveform is nearly identical
                apply_loop_crossfade(&mut loop_buf, quarter_period, CrossfadeMode::Linear);
            } else {
                apply_loop_crossfade(&mut loop_buf, two_periods, CrossfadeMode::CorrelationAdaptive);
            }
        }
        SoundClass::QuasiPeriodic { .. } => {
            // Same as periodic but with longer crossfade and spectral blend fallback
            if candidate.metrics.correlation > 0.8 {
                // Time-domain crossfade
            } else {
                // Spectral blend
                spectral_crossfade(&mut loop_buf, ...);
            }
        }
        SoundClass::Aperiodic => {
            // Split-swap-crossfade — always works
            return split_swap(buffer, config.crossfade_sec.unwrap_or(2.0));
        }
    }
}
```

**Deliverable:** Full pipeline from analysis to output. Soundvault can offer "Make Loopable" as a right-click action, process the file, and replace/create the asset.

---

### Phase 4 — Polish & Extended Features

#### 4a. Batch processing support

```rust
/// Process multiple files with a shared config, returning results per file.
/// Useful for soundvault's "process folder on import" workflow.
pub fn batch_analyze(
    paths: &[PathBuf],
    config: &LoopConfig,
    progress: Option<&dyn Fn(usize, usize)>,  // (current, total)
) -> Vec<Result<LoopAnalysis, LoopError>>;

pub fn batch_make_loop(
    paths: &[PathBuf],
    output_dir: &Path,
    config: &LoopConfig,
    progress: Option<&dyn Fn(usize, usize)>,
) -> Vec<Result<LoopResult, LoopError>>;
```

#### 4b. LUFS loudness normalization (optional)

Add `ebur128` as an optional dependency behind a feature flag:

```toml
[features]
default = []
lufs = ["ebur128"]
```

```rust
#[cfg(feature = "lufs")]
pub fn normalize_lufs(buffer: &mut AudioBuffer, target_lufs: f32);
```

#### 4c. Loopability scoring for soundvault's search index

```rust
/// Quick loopability score (0.0–1.0) without full candidate search.
/// Fast enough to run on import for every file in the library.
pub fn loopability_score(buffer: &AudioBuffer) -> f32;
```

This runs a lightweight version of the analysis: envelope check (does it have a sustained region?), autocorrelation peak strength, spectral stationarity over the middle 50%. Returns a single float that soundvault can store in its index and use as a search/filter/sort field.

#### 4d. Loop verification

```rust
/// Concatenate the loop N times and measure discontinuity artifacts.
/// Returns a quality score and the positions/magnitudes of any detected clicks.
pub fn verify_loop(result: &LoopResult, repetitions: usize) -> LoopVerification;

pub struct LoopVerification {
    pub quality_score: f32,
    pub click_positions: Vec<(usize, f32)>,  // (sample, magnitude)
    pub rms_variation_db: f32,               // RMS deviation across repetitions
}
```

---

## Soundvault Integration Points

The library is designed for these integration patterns:

**On import:** Call `loopability_score()` → store in search index. Fast, non-blocking.

**On asset detail view:** Call `analyze()` → display sound classification, top candidates with scores, detected attack/sustain/release regions. Show waveform with loop point markers.

**On user action ("Make Loopable"):** Call `make_loop()` with user-selected candidate (or best) → write output with `write_wav()` → update library entry.

**On batch import:** Call `batch_analyze()` with progress callback → flag assets that score above threshold as "loop-ready" → optionally auto-process with `batch_make_loop()`.

**Split-swap shortcut:** For ambient files the user tags as "ambience", offer a direct `split_swap()` action that skips analysis entirely — it always works.

---

## Implementation Order & Estimates

| Phase | What | Depends On | Estimated Effort |
|-------|------|------------|-----------------|
| **1a** | WAV smpl chunk read/write | — | 1–2 days |
| **1b** | Peak normalization | types | 0.5 day |
| **1c** | Silence trimming | envelope basics | 0.5 day |
| **1d** | Crossfade curves (all 3 types) | — | 1 day |
| **2a** | Autocorrelation + AMDF | realfft | 1–2 days |
| **2b** | Sound classification | 2a | 0.5 day |
| **2c** | Envelope follower + attack detection | — | 1 day |
| **2d** | Zero-crossing detection | — | 0.5 day |
| **2e** | Spectral features (MFCC, chroma) | realfft | 2–3 days |
| **2f** | Loop candidate finder + scoring | 2a–2e | 2–3 days |
| **3a** | Split-swap-crossfade | 1d | 0.5 day |
| **3b** | Spectral blend | 2e, realfft | 2–3 days |
| **3c** | make_loop orchestrator | 2f, 3a, 3b, 1d | 1–2 days |
| **4a** | Batch processing | phase 3 | 0.5 day |
| **4b** | Loopability score (fast) | 2a, 2c | 1 day |
| **4c** | Loop verification | phase 3 | 1 day |

**Total: ~16–22 working days across all phases.**

Recommended build order for earliest usable value:

1. Phase 1 (foundation) → you can normalize and trim on import immediately
2. Phase 3a (split-swap) → ambient looping works end-to-end
3. Phase 2a–2b (autocorrelation + classification) → analysis starts working
4. Phase 2c–2f (full analysis) → scored candidates
5. Phase 3b–3c (spectral blend + orchestrator) → full auto-looping
6. Phase 4 (polish) → batch processing, verification, loopability index score

---

## Testing Strategy

**Unit tests per module:** Each analysis/processing module gets isolated tests with known-signal inputs (generated sine waves, white noise, chirps). Assert exact sample values where deterministic.

**Integration tests with fixture files:** Small WAV fixtures (~1–2 seconds each):
- `sine_440hz.wav` — pure tone, should classify Periodic, loop at exact period
- `engine_hum.wav` — quasi-periodic, autocorrelation should find approximate period
- `white_noise.wav` — aperiodic, should fall through to split-swap
- `beam_weapon.wav` — tonal sustained, should find pitch-synchronous loop
- `ambient_wind.wav` — aperiodic texture, split-swap should produce clean loop

**Verification test:** For every processed output, run `verify_loop()` and assert `quality_score > 0.9` and zero clicks above threshold.

**Roundtrip test:** Write WAV with smpl chunk → read back → assert loop points match exactly.

---

## Design Decisions & Rationale

**Why no `ndarray` or `nalgebra`?** The math here is 1D signal processing — plain `Vec<f32>` with slice operations is simpler, faster to compile, and has zero learning curve. No matrix operations needed.

**Why `realfft` over `rustfft` directly?** Audio is always real-valued. `realfft` wraps `rustfft` and gives ~2x speedup by exploiting Hermitian symmetry. Same underlying engine, less boilerplate.

**Why hand-roll MFCC instead of using a crate?** The Rust ecosystem doesn't have a mature, well-maintained MFCC crate. The implementation is ~100 lines (mel filterbank + log + DCT), well-understood math, and avoids a fragile dependency. Same reasoning for chroma vectors.

**Why `f32` everywhere?** 32-bit float is the standard working format for audio DSP. It matches what DAWs, game engines, and audio APIs use internally. 64-bit offers no audible benefit for this use case and doubles memory bandwidth.

**Why separate `analyze()` and `make_loop()`?** Soundvault needs to show analysis results (candidates, scores, classification) in the UI before the user decides to process. The two-step API supports this preview-then-commit workflow. For batch processing, they can be chained.
