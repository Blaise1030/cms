import { describe, expect, it } from "vitest";
import { frameHref, framePath } from "./frameUrl";

const BASE = "/preview/site-1/";

describe("framePath", () => {
  it("strips the project base and keeps query and hash", () => {
    expect(framePath("http://x/preview/site-1/", BASE)).toBe("/");
    expect(framePath("http://x/preview/site-1", BASE)).toBe("/");
    expect(framePath("http://x/preview/site-1/about/?q=1#top", BASE)).toBe("/about/?q=1#top");
  });

  it("leaves paths outside the base untouched", () => {
    expect(framePath("http://x/elsewhere", BASE)).toBe("/elsewhere");
  });
});

describe("frameHref", () => {
  it("resolves typed paths under the base", () => {
    expect(frameHref("/about", BASE)).toBe("/preview/site-1/about");
    expect(frameHref("about/", BASE)).toBe("/preview/site-1/about/");
    expect(frameHref("  ", BASE)).toBe("/preview/site-1/");
  });
});
