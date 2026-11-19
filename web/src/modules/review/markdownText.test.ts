import { describe, expect, it } from "vitest";
import { splitImageRuns, stripHtmlComments } from "./markdownText";

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

describe("splitImageRuns", () => {
  it("groups neighbouring image lines and keeps text and its order", () => {
    const md = "Test images\n\n![image](a.png)\n\n![image](b.png)\n![x](c.png) ![y](d.png)\n\nAfter";
    expect(splitImageRuns(md)).toEqual([
      { kind: "text", text: "Test images\n" },
      {
        kind: "images",
        images: [
          { alt: "image", url: "a.png" },
          { alt: "image", url: "b.png" },
          { alt: "x", url: "c.png" },
          { alt: "y", url: "d.png" },
        ],
      },
      { kind: "text", text: "After" },
    ]);
  });

  it("keeps a note with its own image and leaves code and inline images alone", () => {
    const md = "### 1. Hero\n![shot](1.png)\n### 2. Footer\n![shot](2.png)\n```md\n![code](c.png)\n```\nSee ![icon](i.png) here";
    const kinds = splitImageRuns(md).map((b) => (b.kind === "images" ? b.images.map((i) => i.url).join(",") : "text"));
    expect(kinds).toEqual(["text", "1.png", "text", "2.png", "text"]);
    const last = splitImageRuns(md).at(-1)!;
    expect(last.kind === "text" && last.text).toContain("![code](c.png)");
    expect(last.kind === "text" && last.text).toContain("See ![icon](i.png) here");
  });
});
