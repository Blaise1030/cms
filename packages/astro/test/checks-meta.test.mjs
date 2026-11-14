import assert from "node:assert/strict";
import { test } from "node:test";
import { parseHTML } from "linkedom";

import { DESCRIPTION_MAX, TITLE_MAX, readPageMeta } from "../src/checks/meta.js";

function meta(html) {
  const { document } = parseHTML(html);
  return readPageMeta(document);
}

const wrap = (body, head = "") =>
  `<!DOCTYPE html><html lang="en"><head>${head}</head><body>${body}</body></html>`;

test("readPageMeta: full SEO snapshot", () => {
  const html = wrap(
    `<h1>Pricing</h1><h1>Extra</h1>
     <script type="application/ld+json">{"@type":"WebPage","name":"Pricing"}</script>
     <script type="application/ld+json">{"@graph":[{"@type":["Organization","WebSite"]}]}</script>`,
    `<title>Acme Pricing</title>
     <meta name="description" content="Plans for teams.">
     <meta name="robots" content="noindex, nofollow">
     <link rel="canonical" href="https://example.com/pricing/">
     <meta property="og:title" content="OG Pricing">
     <meta property="og:description" content="OG desc">
     <meta property="og:image" content="https://example.com/og.png">`,
  );
  const m = meta(html);
  assert.equal(m.title, "Acme Pricing");
  assert.equal(m.titleLength, "Acme Pricing".length);
  assert.equal(m.description, "Plans for teams.");
  assert.equal(m.descriptionLength, "Plans for teams.".length);
  assert.equal(m.canonical, "https://example.com/pricing/");
  assert.equal(m.robots, "noindex, nofollow");
  assert.equal(m.lang, "en");
  assert.equal(m.h1, "Pricing");
  assert.equal(m.h1Count, 2);
  assert.equal(m.ogTitle, "OG Pricing");
  assert.equal(m.ogDescription, "OG desc");
  assert.equal(m.ogImage, "https://example.com/og.png");
  assert.deepEqual(m.jsonLdTypes, ["WebPage", "Organization", "WebSite"]);
  assert.equal(m.jsonLdBlocks.length, 2);
  assert.match(m.jsonLdBlocks[0], /"@type": "WebPage"/);
});

test("readPageMeta: missing fields use empty strings and default robots", () => {
  const html = `<!DOCTYPE html><html><head></head><body><p>x</p></body></html>`;
  const m = meta(html);
  assert.equal(m.title, "");
  assert.equal(m.titleLength, 0);
  assert.equal(m.description, "");
  assert.equal(m.descriptionLength, 0);
  assert.equal(m.canonical, "");
  assert.equal(m.robots, "index, follow");
  assert.equal(m.lang, "");
  assert.equal(m.h1, "");
  assert.equal(m.h1Count, 0);
  assert.equal(m.ogTitle, "");
  assert.equal(m.ogDescription, "");
  assert.equal(m.ogImage, "");
  assert.deepEqual(m.jsonLdTypes, []);
  assert.deepEqual(m.jsonLdBlocks, []);
});

test("readPageMeta: exports length caps used by the panel", () => {
  assert.equal(TITLE_MAX, 60);
  assert.equal(DESCRIPTION_MAX, 160);
});
