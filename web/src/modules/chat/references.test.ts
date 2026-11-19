import { describe, expect, it } from "vitest";
import { referenceLabel, segmentReferences, selectionPrompt, titleParts } from "./references";

const hero =
  '[element <section class="mx-auto max-w-5xl px-8 py-24 text-center" id="hero"> "Stop building. Start launching." in src/components/sections/Hero.astro:5:69]';
const shot = "/Users/me/orso-projects/.orso/astro-marketing/attachments/4b788490f55975c0.png";

describe("referenceLabel", () => {
  it("labels an element reference by tag and id", () => {
    expect(referenceLabel(`${hero} ${shot}`)).toBe("section#hero");
  });

  it("falls back to the first class, then the tag", () => {
    expect(referenceLabel('[element <h1 class="title big"> "Hi" at `h1`]')).toBe("h1.title");
    expect(referenceLabel("[element <p> at `p`]")).toBe("p");
  });

  it("names anything it cannot parse as an element", () => {
    expect(referenceLabel("Make the hero bolder")).toBe("element");
  });
});

describe("segmentReferences", () => {
  it("turns an element reference and its screenshot into one element segment", () => {
    const text = `${hero} ${shot} make it shorter`;
    expect(segmentReferences(text)).toEqual([
      { kind: "element", label: "section#hero", full: `${hero} ${shot}`, screenshot: true, icon: "i-hugeicons-cursor-pointer-01" },
      { kind: "text", text: " make it shorter" },
    ]);
  });

  it("labels by tag and first class when there is no id, and handles escaped spaces in paths", () => {
    const ref = '[element <h1 class="title big"> "Hi" at `h1`]';
    const path = "/my\\ state/p/attachments/a.png";
    expect(segmentReferences(`Before ${ref} ${path}`)).toEqual([
      { kind: "text", text: "Before " },
      { kind: "element", label: "h1.title", full: `${ref} ${path}`, screenshot: true, icon: "i-hugeicons-cursor-pointer-01" },
    ]);
  });

  it("keeps a reference without a screenshot, and plain text as is", () => {
    expect(segmentReferences("[element <p> at `p`]")).toEqual([
      { kind: "element", label: "p", full: "[element <p> at `p`]", screenshot: false, icon: "i-hugeicons-cursor-pointer-01" },
    ]);
    expect(segmentReferences("Just words [not an element]")).toEqual([
      { kind: "text", text: "Just words [not an element]" },
    ]);
  });
});

describe("titleParts", () => {
  it("turns complete and cut-off element references into labels", () => {
    expect(titleParts('[element <div class="page hero">] Please move the menu')).toEqual({
      elements: ["div.page"],
      text: "Please move the menu",
    });
    expect(titleParts('[element <div class="rel…')).toEqual({ elements: ["div"], text: "" });
    expect(titleParts('[element <img src="/preview/x.png" class="logo"')).toEqual({ elements: ["img.logo"], text: "" });
    expect(titleParts('[element <section id="hero" class="x"')).toEqual({ elements: ["section#hero"], text: "" });
    expect(titleParts("We would need a download button")).toEqual({
      elements: [],
      text: "We would need a download button",
    });
  });
});

describe("selection references", () => {
  it("round-trips a selection, quotes included, into a file-and-words badge", () => {
    const full = selectionPrompt("site/src/content/blog/design.md", 'The designer says "ship it" \\ now');
    expect(full).toBe('[selection in site/src/content/blog/design.md: "The designer says \\"ship it\\" \\\\ now"]');
    const segs = segmentReferences(`Make this punchier ${full} please`);
    expect(segs.map((s) => s.kind)).toEqual(["text", "element", "text"]);
    const badge = segs[1] as Extract<(typeof segs)[number], { kind: "element" }>;
    expect(badge.label).toBe("design.md “The designer says \"ship …”");
    expect(badge.icon).toBe("i-hugeicons-text-selection");
    expect(referenceLabel(full)).toBe(badge.label);
  });

  it("clips long selections", () => {
    expect(selectionPrompt("a.md", "x".repeat(700)).length).toBeLessThan(640);
  });

  it("labels a title cut inside a selection by its file", () => {
    expect(titleParts('[selection in site/blog/design.md: "The desig')).toEqual({ elements: ["design.md"], text: "" });
  });
});
