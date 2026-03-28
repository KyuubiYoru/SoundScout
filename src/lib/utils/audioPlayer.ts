/** Web Audio playback for raw interleaved `f32` PCM. */
export class AudioPlayer {
  private context: AudioContext | null = null;
  private gain: GainNode | null = null;
  private sourceNode: AudioBufferSourceNode | null = null;
  private buffer: AudioBuffer | null = null;
  private startTime = 0;
  private pauseOffset = 0;
  private loop = false;
  isPlaying = false;
  duration = 0;

  setLoop(enabled: boolean): void {
    this.loop = enabled;
  }

  setVolume(v: number): void {
    const g = Math.max(0, Math.min(1, v));
    if (this.gain) {
      this.gain.gain.value = g;
    }
  }

  get currentTime(): number {
    if (!this.context || !this.isPlaying) return this.pauseOffset;
    return this.context.currentTime - this.startTime + this.pauseOffset;
  }

  load(arrayBuffer: ArrayBuffer, sampleRate: number, channels: number): void {
    if (!this.context) {
      this.context = new AudioContext({ sampleRate });
      this.gain = this.context.createGain();
      this.gain.gain.value = 1;
      this.gain.connect(this.context.destination);
    }
    const bytes = new Uint8Array(arrayBuffer);
    const f32 = new Float32Array(bytes.buffer, bytes.byteOffset, bytes.byteLength / 4);
    const frames = f32.length / channels;
    const audioBuf = this.context.createBuffer(channels, frames, sampleRate);
    for (let c = 0; c < channels; c++) {
      const ch = audioBuf.getChannelData(c);
      for (let i = 0; i < frames; i++) {
        ch[i] = f32[i * channels + c] ?? 0;
      }
    }
    this.buffer = audioBuf;
    this.duration = audioBuf.duration;
    this.pauseOffset = 0;
    this.isPlaying = false;
  }

  play(): void {
    if (!this.context || !this.buffer) return;
    this.stopSource();
    const src = this.context.createBufferSource();
    src.buffer = this.buffer;
    const out = this.gain ?? this.context.destination;
    src.connect(out);
    this.startTime = this.context.currentTime;
    src.start(0, this.pauseOffset);
    this.sourceNode = src;
    this.isPlaying = true;
    src.onended = () => {
      if (this.sourceNode !== src) return;
      if (this.loop && this.buffer) {
        this.sourceNode = null;
        this.pauseOffset = 0;
        this.play();
        return;
      }
      this.isPlaying = false;
      this.pauseOffset = 0;
      this.sourceNode = null;
    };
  }

  pause(): void {
    if (!this.context || !this.isPlaying) return;
    this.pauseOffset = this.currentTime;
    this.stopSource();
    this.isPlaying = false;
  }

  seek(position: number): void {
    this.pauseOffset = Math.max(0, Math.min(position, this.duration));
    if (this.isPlaying) {
      this.play();
    }
  }

  stop(): void {
    this.stopSource();
    this.pauseOffset = 0;
    this.isPlaying = false;
  }

  destroy(): void {
    this.stop();
    void this.context?.close();
    this.context = null;
    this.gain = null;
    this.buffer = null;
  }

  private stopSource(): void {
    try {
      this.sourceNode?.stop();
    } catch {
      /* already stopped */
    }
    this.sourceNode = null;
  }
}
