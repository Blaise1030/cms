// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { DESCRIPTION_MAX, TITLE_MAX, readPageMeta } from "./meta";
import { auditDocument } from "./auditDocument";

function doc(html: string): Document {
  return new DOMParser().parseFromString(html, "text/html");
}

describe("readPageMeta", () => {
  it("reads SEO fields and defaults robots", () => {
    const m = readPageMeta(
      doc(`<!DOCTYPE html><html lang="en"><head>
        <title>Hello</title>
        <meta name="description" content="World">
        <link rel="canonical" href="https://example.com/">
        <link rel="icon" href="/favicon.svg">
        <meta property="og:title" content="OG">
        <meta property="og:image" content="/og.png">
      </head><body><h1>Hi</h1>
        <script type="application/ld+json">{"@type":"WebPage"}</script>
      </body></html>`),
      { baseUrl: "https://example.com/blog/" },
    );
    expect(m.title).toBe("Hello");
    expect(m.description).toBe("World");
    expect(m.canonical).toBe("/");
    expect(m.robots).toBe("index, follow");
    expect(m.lang).toBe("en");
    expect(m.h1).toBe("Hi");
    expect(m.ogTitle).toBe("OG");
    expect(m.favicon).toBe("/favicon.svg");
    expect(m.ogImage).toBe("/og.png");
    expect(m.jsonLdTypes).toEqual(["WebPage"]);
    expect(m.jsonLdBlocks).toHaveLength(1);
    expect(m.jsonLdBlocks[0]).toContain('"@type": "WebPage"');
    expect(TITLE_MAX).toBe(60);
    expect(DESCRIPTION_MAX).toBe(160);
  });

  it("strips the Orso preview mount from canonical and asset paths", () => {
    const page = "http://127.0.0.1:51995/preview/astro-marketing/blog/";
    const m = readPageMeta(
      doc(`<!DOCTYPE html><html><head>
        <link rel="canonical" href="https://astro-marketing.blaisetiong-dev.workers.dev/preview/astro-marketing/">
        <link rel="icon" href="/preview/astro-marketing/favicon.svg">
        <meta property="og:image" content="https://astro-marketing.blaisetiong-dev.workers.dev/preview/astro-marketing/og.png">
      </head><body></body></html>`),
      { baseUrl: page },
    );
    expect(m.canonical).toBe("/");
    expect(m.favicon).toBe("/favicon.svg");
    expect(m.ogImage).toBe("/og.png");
  });
});

describe("auditDocument", () => {
  it("returns findings and resources", () => {
    const result = auditDocument(
      doc(`<!DOCTYPE html><html><head></head><body><p>x</p></body></html>`),
      "/",
    );
    expect(result.path).toBe("/");
    expect(result.findings.some((f) => f.rule === "title-missing")).toBe(true);
    expect(result.resources.scripts).toBeGreaterThanOrEqual(0);
    expect(result.meta.title).toBe("");
  });
});
