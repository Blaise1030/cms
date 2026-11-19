import { describe, expect, it } from "vitest";
import { stripHtmlComments } from "./markdownText";

describe("stripHtmlComments", () => {
  it("hides comments, on one line or several, as GitHub does", () => {
    expect(stripHtmlComments("<!-- orso-site-preview-deploy -->\nPreview ready")).toBe("\nPreview ready");
    expect(stripHtmlComments("a <!-- x --> b <!--\nmulti\nline\n--> c")).toBe("a  b  c");
  });

  it("leaves comments inside inline and fenced code alone", () => {
    expect(stripHtmlComments("Use `<!-- marker -->` to tag it <!-- gone -->")).toBe("Use `<!-- marker -->` to tag it ");
    expect(stripHtmlComments("``a `<!-- b -->` c``")).toBe("``a `<!-- b -->` c``");
    const fenced = "```html\n<!-- kept -->\n<p>x</p>\n```\n<!-- dropped -->after";
    expect(stripHtmlComments(fenced)).toBe("```html\n<!-- kept -->\n<p>x</p>\n```\nafter");
    expect(stripHtmlComments("~~~\n<!-- kept -->\n~~~")).toBe("~~~\n<!-- kept -->\n~~~");
  });

  it("leaves text without comments as it is", () => {
    expect(stripHtmlComments("plain `code`")).toBe("plain `code`");
  });
});
