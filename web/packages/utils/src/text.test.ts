import { describe, expect, it } from "vitest";
import { humanize, humanizeLower } from "./text";

describe("humanize", () => {
  it.each([
    ["coverAlt", "Cover alt"],
    ["pubDate", "Pub date"],
    ["relatedPosts", "Related posts"],
    ["title", "Title"],
    ["seo_title", "Seo title"],
    ["hero-image", "Hero image"],
    ["imageURL", "Image URL"],
    ["SEOTitle", "SEO title"],
    ["h1", "H1"],
    ["", ""],
  ])("%s → %s", (name, label) => expect(humanize(name)).toBe(label));

  it("lowercases the first word for mid-sentence use, keeping acronyms", () => {
    expect(humanizeLower("coverAlt")).toBe("cover alt");
    expect(humanizeLower("URLPath")).toBe("URL path");
  });
});
