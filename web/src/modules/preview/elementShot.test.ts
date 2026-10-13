import { describe, expect, it } from "vitest";
import { shotScale } from "./elementShot";

describe("shotScale", () => {
  it("keeps the device pixel ratio for elements that fit", () => {
    expect(shotScale(800, 400, 2)).toBe(2);
    expect(shotScale(800, 400, 0.5)).toBe(1);
  });

  it("shrinks tall sections so the longest side stays bounded", () => {
    expect(shotScale(1200, 8000, 2)).toBe(0.25);
  });
});
