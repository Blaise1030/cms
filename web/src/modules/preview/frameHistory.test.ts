import { describe, expect, it } from "vitest";
import { frameHref, framePath, useFrameHistory } from "./frameHistory";

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

describe("useFrameHistory", () => {
  it("pushes new pages and ignores reloads of the current one", () => {
    const h = useFrameHistory();
    h.visit("/");
    h.visit("/");
    h.visit("/about/");
    expect(h.entries.value).toEqual(["/", "/about/"]);
    expect(h.current.value).toBe("/about/");
    expect(h.canGoBack.value).toBe(true);
    expect(h.canGoForward.value).toBe(false);
  });

  it("moves through the stack on back/forward and truncates on a new visit", () => {
    const h = useFrameHistory();
    h.visit("/");
    h.visit("/a");
    h.visit("/b");
    expect(h.back()).toBe(true);
    h.visit("/a");
    expect(h.current.value).toBe("/a");
    expect(h.canGoForward.value).toBe(true);
    expect(h.forward()).toBe(true);
    h.visit("/b");
    expect(h.current.value).toBe("/b");
    h.back();
    h.visit("/a");
    h.visit("/c");
    expect(h.entries.value).toEqual(["/", "/a", "/c"]);
    expect(h.canGoForward.value).toBe(false);
  });

  it("refuses to go back past the first frame page", () => {
    const h = useFrameHistory();
    h.visit("/");
    expect(h.back()).toBe(false);
    expect(h.forward()).toBe(false);
  });
});
