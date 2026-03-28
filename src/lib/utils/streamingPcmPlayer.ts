/** Web Audio playback for chunked raw interleaved `f32` PCM (gapless scheduling). */

type Queued = { ab: ArrayBuffer; sampleRate: number; channels: number };

export class StreamingPcmPlayer {
  private ctx: AudioContext | null = null;
  private gain: GainNode | null = null;
  private sources: AudioBufferSourceNode[] = [];
  private tailTime = 0;
  private anchorTime = 0;
  private started = false;
  private suspended = false;
  private queue: Queued[] = [];
  private logicalAtPause = 0;
  durationTotal = 0;
  decodeFinished = false;
  private activeSources = 0;
  isPlaying = false;
  /** Fired once when decode is done and the last scheduled buffer has finished (not when paused). */
  onNaturalComplete?: () => void;
  private naturalCompleteFired = false;

  setVolume(v: number): void {
    const g = Math.max(0, Math.min(1, v));
    if (this.gain) {
      this.gain.gain.value = g;
    }
  }

  begin(sampleRate: number, _channels: number, durationTotalSec: number): void {
    this.durationTotal = durationTotalSec;
    this.ctx = new AudioContext({ sampleRate });
    this.gain = this.ctx.createGain();
    this.gain.gain.value = 1;
    this.gain.connect(this.ctx.destination);
    this.tailTime = 0;
    this.anchorTime = 0;
    this.started = false;
    this.suspended = false;
    this.queue = [];
    this.sources = [];
    this.decodeFinished = false;
    this.activeSources = 0;
    this.isPlaying = false;
    this.logicalAtPause = 0;
    this.naturalCompleteFired = false;
  }

  private interleaveToBuffer(f32: Float32Array, sampleRate: number, channels: number): AudioBuffer {
    const ctx = this.ctx!;
    const frames = f32.length / channels;
    const buf = ctx.createBuffer(channels, frames, sampleRate);
    for (let c = 0; c < channels; c++) {
      const ch = buf.getChannelData(c);
      for (let i = 0; i < frames; i++) {
        ch[i] = f32[i * channels + c] ?? 0;
      }
    }
    return buf;
  }

  appendChunk(arrayBuffer: ArrayBuffer, sampleRate: number, channels: number): void {
    if (!this.ctx) return;
    const bytes = new Uint8Array(arrayBuffer);
    if ((bytes.byteLength & 3) !== 0) return;
    const f32 = new Float32Array(bytes.buffer, bytes.byteOffset, bytes.byteLength >> 2);
    if (f32.length === 0) return;

    if (this.suspended) {
      this.queue.push({ ab: arrayBuffer.slice(0), sampleRate, channels });
      return;
    }
    this.scheduleF32(f32, sampleRate, channels);
  }

  private scheduleF32(f32: Float32Array, sampleRate: number, channels: number): void {
    if (!this.ctx) return;
    const buf = this.interleaveToBuffer(f32, sampleRate, channels);
    const src = this.ctx.createBufferSource();
    src.buffer = buf;
    const out = this.gain ?? this.ctx.destination;
    src.connect(out);
    const lead = 0.08;
    if (!this.started) {
      const t0 = this.ctx.currentTime + lead;
      this.anchorTime = t0;
      this.tailTime = t0;
      this.started = true;
    }
    const when = Math.max(this.tailTime, this.ctx.currentTime + 0.02);
    src.start(when);
    this.tailTime = when + buf.duration;
    this.activeSources++;
    this.isPlaying = true;
    src.onended = () => {
      this.activeSources = Math.max(0, this.activeSources - 1);
      if (this.activeSources === 0 && this.decodeFinished) {
        this.isPlaying = false;
        this.fireNaturalCompleteIfNeeded();
      }
    };
    this.sources.push(src);
    void this.ctx.resume();
  }

  private flushQueue(): void {
    for (const q of this.queue) {
      const u = new Uint8Array(q.ab);
      if ((u.byteLength & 3) !== 0) continue;
      const f32 = new Float32Array(u.buffer, u.byteOffset, u.byteLength >> 2);
      this.scheduleF32(f32, q.sampleRate, q.channels);
    }
    this.queue = [];
  }

  markDecodeFinished(): void {
    this.decodeFinished = true;
    if (this.activeSources === 0) {
      this.isPlaying = false;
      this.fireNaturalCompleteIfNeeded();
    }
  }

  private fireNaturalCompleteIfNeeded(): void {
    if (
      this.naturalCompleteFired ||
      this.suspended ||
      !this.decodeFinished ||
      this.activeSources !== 0 ||
      !this.onNaturalComplete
    ) {
      return;
    }
    this.naturalCompleteFired = true;
    this.onNaturalComplete();
  }

  async ensureRunning(): Promise<void> {
    if (this.ctx?.state === "suspended") {
      await this.ctx.resume();
    }
  }

  pause(): void {
    if (this.ctx && this.started && !this.suspended) {
      this.logicalAtPause = Math.min(
        this.durationTotal > 0 ? this.durationTotal : Number.POSITIVE_INFINITY,
        Math.max(0, this.ctx.currentTime - this.anchorTime),
      );
    }
    this.suspended = true;
    void this.ctx?.suspend();
  }

  resume(): void {
    this.suspended = false;
    this.flushQueue();
    void this.ctx?.resume();
  }

  stop(): void {
    for (const s of this.sources) {
      try {
        s.stop();
      } catch {
        /* already stopped */
      }
    }
    this.sources = [];
    this.queue = [];
    this.started = false;
    this.activeSources = 0;
    this.isPlaying = false;
    this.suspended = false;
    this.logicalAtPause = 0;
    this.naturalCompleteFired = false;
    void this.ctx?.close();
    this.ctx = null;
    this.gain = null;
  }

  get currentTime(): number {
    if (!this.ctx || !this.started) return 0;
    if (this.suspended) {
      return this.logicalAtPause;
    }
    const t = this.ctx.currentTime - this.anchorTime;
    const cap = this.durationTotal > 0 ? this.durationTotal : t;
    return Math.max(0, Math.min(cap, t));
  }
}
