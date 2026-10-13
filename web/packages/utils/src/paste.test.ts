import { describe, expect, it } from "vitest";
import { bracketedPaste } from "./paste";

describe("bracketedPaste", () => {
  it("wraps text in bracketed paste markers", () => {
    expect(bracketedPaste("hi")).toBe("\x1b[200~hi\x1b[201~");
  });
  it("strips an embedded end marker", () => {
    expect(bracketedPaste("a\x1b[201~b")).toBe("\x1b[200~ab\x1b[201~");
  });
});
