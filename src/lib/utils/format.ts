/** `ms` as `M:SS.S`, `H:MM:SS.S`, or `—`. */
export function formatDuration(ms: number | null | undefined): string {
  if (ms == null) return "—";
  const totalSec = ms / 1000;
  const fracDigit = Math.min(9, Math.round((totalSec % 1) * 10));
  const wholeSec = Math.floor(totalSec);

  if (wholeSec < 60) {
    return `0:${String(wholeSec).padStart(2, "0")}.${fracDigit}`;
  }
  if (wholeSec < 3600) {
    const m = Math.floor(wholeSec / 60);
    const s = wholeSec % 60;
    return `${m}:${String(s).padStart(2, "0")}.${fracDigit}`;
  }
  const h = Math.floor(wholeSec / 3600);
  const m = Math.floor((wholeSec % 3600) / 60);
  const s = wholeSec % 60;
  return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${fracDigit}`;
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function formatSampleRate(rate: number): string {
  if (rate === 44100) return "44.1 kHz";
  if (rate === 48000) return "48 kHz";
  if (rate === 96000) return "96 kHz";
  if (rate === 22050) return "22.05 kHz";
  if (rate >= 1000) return `${(rate / 1000).toFixed(1)} kHz`;
  return `${rate} Hz`;
}
