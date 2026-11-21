import { describe, expect, it } from "vitest";
import { useFrameHistory } from "./frameHistory";

describe("useFrameHistory target", () => {
  it("names the entry one step away, or null at the ends", () => {
    const h = useFrameHistory();
    h.visit("/");
    h.visit("/blog");
    h.visit("/blog/post");
    expect(h.target(-1)).toBe("/blog");
    expect(h.target(1)).toBeNull();
    h.back(); h.visit("/blog");
    expect(h.target(-1)).toBe("/");
    expect(h.target(1)).toBe("/blog/post");
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

  it("ignores a late report from the page being left while a step is on its way", () => {
    const h = useFrameHistory();
    h.visit("/");
    h.visit("/a");
    h.visit("/b");
    expect(h.back()).toBe(true);
    // The old page answers a handshake after Back was pressed.
    h.visit("/b");
    expect(h.current.value).toBe("/b");
    h.visit("/a");
    expect(h.entries.value).toEqual(["/", "/a", "/b"]);
    expect(h.current.value).toBe("/a");
  });

  it("refuses to go back past the first frame page", () => {
    const h = useFrameHistory();
    h.visit("/");
    expect(h.back()).toBe(false);
    expect(h.forward()).toBe(false);
  });

  it("ignores the page being left reporting late after a typed address", () => {
    const h = useFrameHistory();
    h.visit("/");
    h.navigate("/blog/spring/");
    expect(h.current.value).toBe("/blog/spring/");
    // The old page's report arrives after the frame was sent away: not where it is now.
    expect(h.visit("/")).toBe(false);
    expect(h.current.value).toBe("/blog/spring/");
    expect(h.visit("/blog/spring/")).toBe(true);
    expect(h.entries.value).toEqual(["/", "/blog/spring/"]);
    // Once the requested page reported in, going back to the home page is a real visit.
    expect(h.visit("/")).toBe(true);
    expect(h.current.value).toBe("/");
  });

  it("takes a redirect from the requested page in its place", () => {
    const h = useFrameHistory();
    h.visit("/");
    h.navigate("/blog");
    expect(h.visit("/blog/")).toBe(true);
    expect(h.entries.value).toEqual(["/", "/blog/"]);
    expect(h.canGoBack.value).toBe(true);
  });

  it("guards an open-in-preview after a reset against the page it left", () => {
    const h = useFrameHistory();
    h.visit("/help/");
    const leaving = h.current.value;
    h.reset();
    h.navigate("/blog/spring/", leaving);
    expect(h.visit("/help/")).toBe(false);
    expect(h.current.value).toBe("/blog/spring/");
  });

  it("lets a back step land while a typed page has not reported in yet", () => {
    const h = useFrameHistory();
    h.visit("/");
    h.navigate("/blog");
    h.visit("/blog");
    h.navigate("/help");
    // Back before /help reported: its arrival on /blog is a real step, not a late report.
    expect(h.back()).toBe(true);
    expect(h.visit("/blog")).toBe(true);
    expect(h.current.value).toBe("/blog");
  });
});
