import { describe, expect, it } from "vitest";
import {
  closestNormalizePeakDbfs,
  defaultPostProcessConfig,
  peakDbfsToLinear,
  linearPeakToDbfs,
} from "../src/lib/types/audio";

describe("peakDbfsToLinear", () => {
  it("maps 0 dBFS to unity", () => {
    expect(peakDbfsToLinear(0)).toBeCloseTo(1, 6);
  });

  it("maps -6 dBFS to default linear peak (~0.501)", () => {
    const lin = peakDbfsToLinear(-6);
    expect(lin).toBeCloseTo(0.501187234, 5);
    expect(defaultPostProcessConfig.normalizeTarget).toBeCloseTo(lin, 10);
  });
});

describe("linearPeakToDbfs", () => {
  it("round-trips with peakDbfsToLinear", () => {
    expect(linearPeakToDbfs(peakDbfsToLinear(-6))).toBeCloseTo(-6, 5);
  });
});

describe("closestNormalizePeakDbfs", () => {
  it("picks -6 for default target", () => {
    expect(closestNormalizePeakDbfs(defaultPostProcessConfig.normalizeTarget)).toBe(-6);
  });

  it("maps legacy ~0.97 linear toward 0 dBFS", () => {
    expect(closestNormalizePeakDbfs(0.97)).toBe(0);
  });
});
