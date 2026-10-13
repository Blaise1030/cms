import { describe, expect, it } from "vitest";
import { useFrameHistory } from "./frameHistory";

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
