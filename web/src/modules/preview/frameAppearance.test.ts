// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { applyFrameAppearance } from "./frameAppearance";

function doc(html = "<html><body></body></html>") {
  return new DOMParser().parseFromString(html, "text/html");
}

describe("applyFrameAppearance", () => {
  it("sets and clears dir and the dark class", () => {
    const d = doc();
    const root = d.documentElement;
    applyFrameAppearance(d, { rtl: true, dark: true });
    expect(root.getAttribute("dir")).toBe("rtl");
    expect(root.classList.contains("dark")).toBe(true);
    applyFrameAppearance(d, { rtl: false, dark: false });
    expect(root.hasAttribute("dir")).toBe(false);
    expect(root.classList.contains("dark")).toBe(false);
  });

  it("restores the page's own dir and dark class", () => {
    const d = doc('<html dir="ltr" class="dark"><body></body></html>');
    const root = d.documentElement;
    applyFrameAppearance(d, { rtl: true, dark: false });
    expect(root.getAttribute("dir")).toBe("rtl");
    expect(root.classList.contains("dark")).toBe(true);
    applyFrameAppearance(d, { rtl: false, dark: false });
    expect(root.getAttribute("dir")).toBe("ltr");
  });

  it("re-reads the page's state when fresh", () => {
    const d = doc();
    const root = d.documentElement;
    applyFrameAppearance(d, { rtl: false, dark: false });
    root.setAttribute("dir", "rtl");
    applyFrameAppearance(d, { rtl: false, dark: false }, true);
    expect(root.getAttribute("dir")).toBe("rtl");
  });
});
