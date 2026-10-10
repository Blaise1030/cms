import { describe, expect, it } from "vitest";
import { languageForPath } from "./language";

describe("languageForPath", () => {
  it("maps common extensions to CodeMirror languages", () => {
    expect(languageForPath("x.ts")).not.toBeNull();
    expect(languageForPath("x.tsx")).not.toBeNull();
    expect(languageForPath("x.js")).not.toBeNull();
    expect(languageForPath("x.json")).not.toBeNull();
    expect(languageForPath("x.md")).not.toBeNull();
    expect(languageForPath("x.py")).not.toBeNull();
    expect(languageForPath("x.rs")).not.toBeNull();
    expect(languageForPath("x.yml")).not.toBeNull();
    expect(languageForPath("x.astro")).not.toBeNull();
  });

  it("returns null for unknown extensions", () => {
    expect(languageForPath("x.bin")).toBeNull();
    expect(languageForPath("Dockerfile")).toBeNull();
  });
});
