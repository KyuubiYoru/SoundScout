import { describe, expect, it } from "vitest";
import {
  CLIP_MIN_SEC,
  clampClipToDuration,
  shouldDeferClipClampForHybrid,
} from "../src/lib/utils/clipBounds";

describe("clampClipToDuration", () => {
  it("returns null for non-positive duration", () => {
    expect(clampClipToDuration({ start: 0, end: 10 }, 0)).toBeNull();
    expect(clampClipToDuration({ start: 0, end: 10 }, -1)).toBeNull();
  });

  it("keeps in-range clip unchanged", () => {
    expect(clampClipToDuration({ start: 10, end: 20 }, 60)).toEqual({ start: 10, end: 20 });
  });

  it("clamps end to duration", () => {
    expect(clampClipToDuration({ start: 0, end: 100 }, 60)).toEqual({ start: 0, end: 60 });
  });

  it("returns null when clamped span collapses below minimum", () => {
    expect(clampClipToDuration({ start: 22, end: 89 }, 5)).toBeNull();
  });

  /**
   * Bug scenario: clip 10–20s on a 60s file; `dur` wrongly uses ~8s (preview PCM length).
   * Clamping destroys the selection (null or bogus range) — tick() must defer until hybrid matches file.
   */
  it("clamping with short dur destroys a full-file clip (defer when hybrid is still preview-sized)", () => {
    const clip = { start: 10, end: 20 };
    const wrongDur = 8;
    const bad = clampClipToDuration(clip, wrongDur);
    expect(bad).not.toEqual(clip);
    expect(bad === null || bad.end <= wrongDur).toBe(true);
  });
});

describe("shouldDeferClipClampForHybrid", () => {
  const clip = { start: 10, end: 20 };

  it("defers when hybrid is short preview buffer but asset is full length", () => {
    expect(shouldDeferClipClampForHybrid(clip, 8, 60)).toBe(true);
    expect(shouldDeferClipClampForHybrid(clip, 15.9, 60)).toBe(true);
  });

  it("does not defer when hybrid matches asset length", () => {
    expect(shouldDeferClipClampForHybrid(clip, 60, 60)).toBe(false);
    expect(shouldDeferClipClampForHybrid(clip, 59.96, 60)).toBe(false);
  });

  it("does not defer when clip fits entirely inside hybrid duration", () => {
    expect(shouldDeferClipClampForHybrid({ start: 1, end: 5 }, 8, 60)).toBe(false);
  });

  it("does not defer when asset duration unknown", () => {
    expect(shouldDeferClipClampForHybrid(clip, 8, 0)).toBe(false);
  });

  it("does not defer for short file where hybrid equals asset", () => {
    expect(shouldDeferClipClampForHybrid({ start: 0, end: 5 }, 5, 5)).toBe(false);
  });
});
