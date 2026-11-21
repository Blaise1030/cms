import { describe, expect, it } from "vitest";
import { elementPrompt, relativeFile } from "./elementPicker";

describe("elementPrompt", () => {
  it("points at the Astro source file", () => {
    const pick = {
      label: "h1.title",
      tag: '<h1 class="title">',
      selector: "body > main > section > h1",
      text: "Build faster sites",
      file: relativeFile("/home/me/orso-projects/site-1/src/components/Hero.astro", "site-1"),
      loc: "12:3",
    };
    expect(pick.file).toBe("src/components/Hero.astro");
    expect(elementPrompt(pick)).toBe(
      '[element <h1 class="title"> "Build faster sites" in src/components/Hero.astro:12:3]',
    );
  });

  it("falls back to the CSS path without source annotations", () => {
    const pick = {
      label: "a",
      tag: '<a href="/buy">',
      selector: "#pricing > ul > li:nth-of-type(2) > a",
      text: "Buy",
    };
    expect(elementPrompt(pick)).toBe(
      '[element <a href="/buy"> "Buy" at `#pricing > ul > li:nth-of-type(2) > a`]',
    );
  });

  it("puts the screenshot path after the reference, escaped, so the agent attaches it", () => {
    const pick = { tag: "<h1>", selector: "h1", text: "Hi", label: "h1", screenshot: "/state/My Site/a.png" };
    expect(elementPrompt(pick)).toBe('[element <h1> "Hi" at `h1`] /state/My\\ Site/a.png ');
  });

  it("keeps paths outside the project as they are", () => {
    expect(relativeFile("/elsewhere/x.astro", "site-1")).toBe("/elsewhere/x.astro");
  });
});
