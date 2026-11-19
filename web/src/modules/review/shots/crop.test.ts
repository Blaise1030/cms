import { describe, expect, it } from "vitest";
import { cropRect, dragRect, isSelection, MAX_SIDE, outputSize } from "./crop";

describe("dragRect", () => {
  it("is the same rectangle whichever way the drag went", () => {
    expect(dragRect(50, 80, 10, 20)).toEqual({ x: 10, y: 20, width: 40, height: 60 });
    expect(isSelection(dragRect(0, 0, 7, 40))).toBe(false);
    expect(isSelection(dragRect(0, 0, 8, 8))).toBe(true);
  });
});

describe("cropRect", () => {
  it("maps CSS pixels to a 2x capture", () => {
    // A 1280x800 window captured at devicePixelRatio 2.
    expect(cropRect({ x: 100, y: 50, width: 200, height: 120 }, { width: 2560, height: 1600 }, { width: 1280, height: 800 })).toEqual(
      { sx: 200, sy: 100, sw: 400, sh: 240 },
    );
  });

  it("maps to a capture the browser scaled down, per axis", () => {
    // The tab was captured smaller than the viewport (e.g. 1920x1200 for a 2400x1500 window).
    expect(cropRect({ x: 240, y: 150, width: 480, height: 300 }, { width: 1920, height: 1200 }, { width: 2400, height: 1500 })).toEqual(
      { sx: 192, sy: 120, sw: 384, sh: 240 },
    );
  });

  it("stays inside the frame", () => {
    expect(cropRect({ x: -10, y: 790, width: 50, height: 50 }, { width: 1280, height: 800 }, { width: 1280, height: 800 })).toEqual(
      { sx: 0, sy: 790, sw: 50, sh: 10 },
    );
  });
});

describe("outputSize", () => {
  it("keeps small crops and shrinks large ones to 1600 on the longest side", () => {
    expect(outputSize(400, 240)).toEqual({ width: 400, height: 240 });
    expect(outputSize(3200, 1000)).toEqual({ width: MAX_SIDE, height: 500 });
    expect(outputSize(800, 4000)).toEqual({ width: 320, height: MAX_SIDE });
  });
});
