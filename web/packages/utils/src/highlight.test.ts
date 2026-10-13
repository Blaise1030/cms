import { describe, expect, it } from "vitest";
import { escapeHtml, highlightCode, langForPath } from "./highlight";

describe("highlight", () => {
  it("maps extensions to languages", () => {
    expect(langForPath("src/pages/index.astro")).toBe("astro");
    expect(langForPath("a/b/c.MDX")).toBe("mdx");
    expect(langForPath("package.json")).toBe("json");
    expect(langForPath("LICENSE")).toBe("text");
  });

  it("escapes html for the plain fallback", () => {
    expect(escapeHtml("<a & b>")).toBe("&lt;a &amp; b&gt;");
  });

  it("renders shiki output for real code", async () => {
    const html = await highlightCode("const x = 1;", "x.ts");
    expect(html).toContain("shiki");
    expect(html).toContain("const");
  });

  it("falls back to escaped text past the size limit", async () => {
    const html = await highlightCode("<".repeat(200_001), "x.ts");
    expect(html).toContain("&lt;");
    expect(html).not.toContain("<span");
  });
});
