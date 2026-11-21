import { describe, expect, it } from "vitest";
import { sectionIdForFile } from "./conflictHighlight";

describe("conflict section highlighting", () => {
  it("names the section a section file builds", () => {
    expect(sectionIdForFile("src/content/sections/hero.mdx")).toBe("hero");
  });

  it("falls back to whole page for shared files", () => {
    expect(sectionIdForFile("src/styles/global.css")).toBeNull();
    expect(sectionIdForFile("src/content/sections/Hero.mdx")).toBeNull();
  });
});
