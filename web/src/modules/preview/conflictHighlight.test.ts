// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { attachConflictHighlight, findConflictTarget, sectionIdForFile } from "./conflictHighlight";

beforeEach(() => { document.body.innerHTML = ""; });

describe("conflict section highlighting", () => {
  it("matches a stable section ID after layout order changes", () => {
    document.body.innerHTML = '<section id="features"></section><section id="hero"></section>';
    const target = findConflictTarget(document, "src/content/sections/hero.mdx");
    expect(target?.id).toBe("hero");
    const detach = attachConflictHighlight(window, target);
    expect(document.querySelector("[data-conflict-highlight]")).not.toBeNull();
    detach();
    expect(document.querySelector("[data-conflict-highlight]")).toBeNull();
  });

  it("falls back to whole page for shared styles, absent and ambiguous sections", () => {
    expect(sectionIdForFile("src/styles/global.css")).toBeNull();
    expect(findConflictTarget(document, "src/styles/global.css")).toBeNull();
    expect(findConflictTarget(document, "src/content/sections/hero.mdx")).toBeNull();
    document.body.innerHTML = '<section id="hero"></section><section id="hero"></section>';
    expect(findConflictTarget(document, "src/content/sections/hero.mdx")).toBeNull();
  });
});
