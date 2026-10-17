import { describe, expect, it } from "vitest";
import { expandReferences, referenceTag, segmentReferences } from "./references";

const hero =
  '[element <section class="mx-auto max-w-5xl px-8 py-24 text-center" id="hero"> "Stop building. Start launching." in src/components/sections/Hero.astro:5:69]';
const shot = "/Users/me/workbench-projects/.workbench/astro-marketing/attachments/4b788490f55975c0.png";

describe("referenceTag", () => {
  it("numbers element tags", () => {
    expect(referenceTag(1)).toBe("[Element #1]");
    expect(referenceTag(12)).toBe("[Element #12]");
  });
});

describe("expandReferences", () => {
  it("replaces each tag with the text it stands for", () => {
    const refs = new Map([
      ["[Element #1]", `${hero} ${shot}`],
      ["[Element #2]", "[element <h1> at `h1`]"],
    ]);
    expect(expandReferences("Make [Element #1] bolder, like [Element #2]", refs)).toBe(
      `Make ${hero} ${shot} bolder, like [element <h1> at \`h1\`]`,
    );
  });

  it("leaves text without tags, and tags it doesn't know, unchanged", () => {
    const refs = new Map([["[Element #1]", hero]]);
    expect(expandReferences("Fix the typo", refs)).toBe("Fix the typo");
    expect(expandReferences("See [Element #3]", refs)).toBe("See [Element #3]");
  });
});

describe("segmentReferences", () => {
  it("turns an element reference and its screenshot into one element segment", () => {
    const text = `${hero} ${shot} make it shorter`;
    expect(segmentReferences(text)).toEqual([
      { kind: "element", label: "section#hero", full: `${hero} ${shot}`, screenshot: true },
      { kind: "text", text: " make it shorter" },
    ]);
  });

  it("labels by tag and first class when there is no id, and handles escaped spaces in paths", () => {
    const ref = '[element <h1 class="title big"> "Hi" at `h1`]';
    const path = "/my\\ state/p/attachments/a.png";
    expect(segmentReferences(`Before ${ref} ${path}`)).toEqual([
      { kind: "text", text: "Before " },
      { kind: "element", label: "h1.title", full: `${ref} ${path}`, screenshot: true },
    ]);
  });

  it("keeps a reference without a screenshot, and plain text as is", () => {
    expect(segmentReferences("[element <p> at `p`]")).toEqual([
      { kind: "element", label: "p", full: "[element <p> at `p`]", screenshot: false },
    ]);
    expect(segmentReferences("Just words [not an element]")).toEqual([
      { kind: "text", text: "Just words [not an element]" },
    ]);
  });
});
