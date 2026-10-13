import { describe, expect, it } from "vitest";
import { DESKTOP_MIN_WIDTH, frameFit } from "./frameFit";

describe("frameFit", () => {
  it("fills a panel at least as wide as a desktop", () => {
    expect(frameFit("desktop", 1600, 900)).toEqual({ width: null, height: null, scale: 1 });
    expect(frameFit("desktop", DESKTOP_MIN_WIDTH, 900)).toEqual({ width: null, height: null, scale: 1 });
  });

  it("lays desktop out at full width and scales it into a narrower panel", () => {
    const fit = frameFit("desktop", 720, 600);
    expect(fit.width).toBe(DESKTOP_MIN_WIDTH);
    expect(fit.scale).toBe(0.5);
    // Scaled back down, the frame is exactly as tall as the panel.
    expect(fit.height! * fit.scale).toBe(600);
  });

  it("keeps fixed device widths, scaling only when the panel is narrower", () => {
    expect(frameFit("tablet", 1000, 800)).toEqual({ width: 768, height: null, scale: 1 });
    expect(frameFit("mobile", 390, 800)).toEqual({ width: 390, height: null, scale: 1 });
    const fit = frameFit("tablet", 384, 800);
    expect(fit).toEqual({ width: 768, height: 1600, scale: 0.5 });
  });

  it("does not scale before the panel is measured", () => {
    expect(frameFit("desktop", 0, 0)).toEqual({ width: null, height: null, scale: 1 });
    expect(frameFit("tablet", 0, 0)).toEqual({ width: 768, height: null, scale: 1 });
  });
});
