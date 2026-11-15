import { describe, expect, it } from "vitest";
import { conflictLabel, conflictSubject } from "./conflictLabels";

describe("conflictLabel", () => {
  it("names sections, content entries and pages instead of paths", () => {
    expect(conflictLabel("src/content/sections/hero.mdx")).toBe("Hero section");
    expect(conflictLabel("src/content/blog/en/launch-week.md")).toBe("Blog · Launch week");
    expect(conflictLabel("src/content/help/getting-started.mdx")).toBe("Help · Getting started");
    expect(conflictLabel("src/pages/index.astro")).toBe("Home page");
    expect(conflictLabel("src/pages/about.astro")).toBe("About page");
    expect(conflictLabel("src/pages/pricing/index.astro")).toBe("Pricing page");
    expect(conflictLabel("index.html")).toBe("Home page");
  });

  it("falls back to the file name for anything else", () => {
    expect(conflictLabel("src/styles/global.css")).toBe("global.css");
    expect(conflictLabel("public/logo.png")).toBe("logo.png");
  });
});

describe("conflictSubject", () => {
  it("names one file, counts several", () => {
    expect(conflictSubject(["src/content/sections/hero.mdx"])).toBe("the Hero section");
    expect(conflictSubject(["a.css", "b.css", "index.html"])).toBe("3 parts of your site");
  });
});
