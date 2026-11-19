import { describe, expect, it } from "vitest";
import { referenceLabel, segmentReferences, titleParts } from "./references";

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
