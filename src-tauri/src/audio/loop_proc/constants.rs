//! Shared FFT size for MFCC-based loop search and [`LoopTechnique::SpectralBlend`](super::LoopTechnique).

/// FFT size for MFCC in loop candidate scoring and spectral seam blend (must stay aligned).
pub(crate) const MFCC_FFT_SIZE: usize = 512;
