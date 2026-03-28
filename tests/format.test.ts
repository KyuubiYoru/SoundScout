import { describe, expect, it } from "vitest";
import { formatDuration, formatFileSize, formatSampleRate } from "../src/lib/utils/format";

describe("formatDuration", () => {
  it("handles null and undefined", () => {
    expect(formatDuration(null)).toBe("—");
    expect(formatDuration(undefined)).toBe("—");
  });
  it("sub-minute", () => {
    expect(formatDuration(0)).toMatch(/^0:00/);
    expect(formatDuration(500)).toMatch(/^0:00/);
    expect(formatDuration(2500)).toContain("02");
  });
  it("minutes", () => {
    expect(formatDuration(125_000)).toMatch(/^2:/);
  });
  it("hours", () => {
    expect(formatDuration(3_661_000)).toMatch(/^1:/);
  });
});

describe("formatFileSize", () => {
  it("units", () => {
    expect(formatFileSize(500)).toContain("B");
    expect(formatFileSize(1536)).toContain("KB");
    expect(formatFileSize(5_242_880)).toContain("MB");
    expect(formatFileSize(1073741824)).toContain("GB");
  });
});

describe("formatSampleRate", () => {
  it("common rates", () => {
    expect(formatSampleRate(44100)).toBe("44.1 kHz");
    expect(formatSampleRate(48000)).toBe("48 kHz");
    expect(formatSampleRate(96000)).toBe("96 kHz");
    expect(formatSampleRate(22050)).toBe("22.05 kHz");
  });
});
