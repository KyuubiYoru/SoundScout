import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { AudioPlayer } from "../src/lib/utils/audioPlayer";

describe("AudioPlayer", () => {
  beforeEach(() => {
    const dest = { connect: vi.fn(), disconnect: vi.fn() } as unknown;
    vi.stubGlobal(
      "AudioContext",
      class {
        sampleRate = 44100;
        currentTime = 0;
        destination = dest;
        createBuffer = (ch: number, len: number, sr: number) => ({
          sampleRate: sr,
          duration: len / sr,
          numberOfChannels: ch,
          getChannelData: () => new Float32Array(len),
        });
        createBufferSource = () => {
          const node = {
            buffer: null as AudioBuffer | null,
            connect: vi.fn(),
            start: vi.fn(),
            onended: null as (() => void) | null,
          };
          return node;
        };
        createGain = () => ({
          gain: { value: 1 },
          connect: vi.fn(),
        });
        close = vi.fn().mockResolvedValue(undefined);
      },
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("load sets duration from buffer", () => {
    const p = new AudioPlayer();
    const samples = new Float32Array([0, 0, 0, 0]);
    const ab = samples.buffer;
    p.load(ab, 44100, 2);
    expect(p.duration).toBeGreaterThan(0);
    p.destroy();
  });
});
