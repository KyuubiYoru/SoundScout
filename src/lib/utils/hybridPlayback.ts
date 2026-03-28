import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import type { Asset } from "$lib/types";
import type { PcmStreamStart } from "$lib/ipc";
import { AudioPlayer } from "./audioPlayer";
import { StreamingPcmPlayer } from "./streamingPcmPlayer";

export type HybridMode = "stream" | "pcm" | "pcm_stream" | "none";

/**
 * Prefer OS/WebView streaming (`<audio>` + `convertFileSrc`).
 * Does not wait for `canplay` — that event often implies large buffers on long files and delays start.
 * Call `play()` right after `prepareStream()` so the engine begins progressive decode immediately.
 */
export class HybridPlayback {
  private readonly el = new Audio();
  private readonly pcm = new AudioPlayer();
  private stream: StreamingPcmPlayer | null = null;
  private streamSessionId: number | null = null;
  /** User preference (full-track loop). Ignored for native/file loop while a clip is active. */
  private userWantsLoop = false;
  /** When true, full-file native loop / stream restart is disabled; app handles bounds. */
  private clipActive = false;
  private onPcmStreamLoop: (() => void) | null = null;
  mode: HybridMode = "none";

  constructor() {
    /* `none` avoids hinting a full read before `play()`; streaming starts on demand. */
    this.el.preload = "none";
  }

  /** Invoked when chunked PCM playback reaches a natural end (for loop restart). */
  setOnPcmStreamLoop(handler: (() => void) | null): void {
    this.onPcmStreamLoop = handler;
    this.applyStreamLoopCallback();
  }

  setLoopPolicy(userLoop: boolean, clipActive: boolean): void {
    this.userWantsLoop = userLoop;
    this.clipActive = clipActive;
    this.applyLoopToEngines();
  }

  private applyLoopToEngines(): void {
    const native = this.userWantsLoop && !this.clipActive;
    if (this.mode === "stream") this.el.loop = native;
    this.pcm.setLoop(native);
    this.applyStreamLoopCallback();
  }

  private applyStreamLoopCallback(): void {
    if (!this.stream) return;
    const loopHandler =
      this.userWantsLoop && !this.clipActive && this.onPcmStreamLoop ? this.onPcmStreamLoop : undefined;
    this.stream.onNaturalComplete = loopHandler;
  }

  static canStream(): boolean {
    return isTauri();
  }

  /** Linear gain 0–1 for all playback modes. */
  setVolume(volume: number): void {
    const clampedGain = Math.max(0, Math.min(1, volume));
    this.el.volume = clampedGain;
    this.pcm.setVolume(clampedGain);
    this.stream?.setVolume(clampedGain);
  }

  /** Attach stream URL and reset element. Synchronous — do not await browser buffering. */
  prepareStream(filePath: string): void {
    this.stopStreamPcm();
    this.pcm.stop();
    this.mode = "stream";
    this.el.pause();
    this.el.preload = "none";
    this.el.src = convertFileSrc(filePath);
    this.el.loop = this.userWantsLoop && !this.clipActive;
    this.el.load();
  }

  loadPcm(arrayBuffer: ArrayBuffer, sampleRate: number, channels: number): void {
    this.stopStreamPcm();
    this.el.pause();
    this.el.removeAttribute("src");
    this.el.load();
    this.mode = "pcm";
    this.pcm.load(arrayBuffer, sampleRate, channels);
  }

  /** Chunked PCM: first buffer already decoded; more chunks arrive via events + `appendPcmStreamChunk`. */
  preparePcmStream(asset: Asset, start: PcmStreamStart, firstChunkAb: ArrayBuffer): void {
    this.stopStreamPcm();
    this.el.pause();
    this.el.removeAttribute("src");
    this.el.load();
    this.mode = "pcm_stream";
    this.streamSessionId = start.streamId;
    this.stream = new StreamingPcmPlayer();
    const durSec =
      asset.durationMs != null
        ? asset.durationMs / 1000
        : start.durationSec > 0
          ? start.durationSec
          : 0;
    this.stream.begin(start.sampleRate, start.channels, durSec);
    this.stream.appendChunk(firstChunkAb, start.sampleRate, start.channels);
    this.applyStreamLoopCallback();
  }

  appendPcmStreamChunk(path: string, sampleRate: number, channels: number, streamId: number): void {
    if (this.mode !== "pcm_stream" || this.streamSessionId !== streamId || !this.stream) {
      return;
    }
    const session = streamId;
    void (async () => {
      const response = await fetch(convertFileSrc(path));
      if (this.mode !== "pcm_stream" || this.streamSessionId !== session || !this.stream) {
        return;
      }
      if (!response.ok) return;
      const ab = await response.arrayBuffer();
      if (this.mode !== "pcm_stream" || this.streamSessionId !== session || !this.stream) {
        return;
      }
      this.stream.appendChunk(ab, sampleRate, channels);
    })();
  }

  markPcmStreamDecodeFinished(): void {
    this.stream?.markDecodeFinished();
  }

  get activePcmStreamId(): number | null {
    return this.streamSessionId;
  }

  private stopStreamPcm(): void {
    this.stream?.stop();
    this.stream = null;
    this.streamSessionId = null;
  }

  get currentTime(): number {
    if (this.mode === "stream") return this.el.currentTime;
    if (this.mode === "pcm_stream" && this.stream) return this.stream.currentTime;
    return this.pcm.currentTime;
  }

  get duration(): number {
    if (this.mode === "stream") {
      const d = this.el.duration;
      return Number.isFinite(d) && d > 0 ? d : 0;
    }
    if (this.mode === "pcm_stream" && this.stream) {
      return this.stream.durationTotal;
    }
    return this.pcm.duration;
  }

  get isPlaying(): boolean {
    if (this.mode === "stream") return !this.el.paused;
    if (this.mode === "pcm_stream" && this.stream) return this.stream.isPlaying;
    return this.pcm.isPlaying;
  }

  async play(): Promise<void> {
    if (this.mode === "stream") {
      await this.el.play();
    } else if (this.mode === "pcm_stream") {
      await this.stream?.ensureRunning();
    } else {
      this.pcm.play();
    }
  }

  pause(): void {
    if (this.mode === "stream") this.el.pause();
    else if (this.mode === "pcm_stream") this.stream?.pause();
    else this.pcm.pause();
  }

  seek(position: number): void {
    if (this.mode === "stream") {
      this.el.currentTime = position;
    } else if (this.mode === "pcm_stream") {
      this.stopStreamPcm();
      this.mode = "none";
    } else {
      this.pcm.seek(position);
    }
  }

  stop(): void {
    if (this.mode === "stream") {
      this.el.pause();
      this.el.currentTime = 0;
      this.el.removeAttribute("src");
      this.el.load();
    } else if (this.mode === "pcm_stream") {
      this.stopStreamPcm();
    } else {
      this.pcm.stop();
    }
    this.mode = "none";
  }

  destroy(): void {
    this.el.pause();
    this.el.removeAttribute("src");
    this.el.load();
    this.pcm.destroy();
    this.stopStreamPcm();
    this.mode = "none";
  }
}
