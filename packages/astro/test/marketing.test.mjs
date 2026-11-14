import assert from "node:assert/strict";
import { test } from "node:test";

import { normalizeCollectionsOption } from "../src/collections-option.js";
import { marketingCollections, marketingPreviewPaths } from "../src/marketing.js";

test("marketingCollections has blog and helpDocs keys", () => {
  assert.deepEqual(Object.keys(marketingCollections).sort(), ["blog", "helpDocs"]);
});

test("marketingCollections sets hasPreviewPath for both collections via normalize", () => {
  const { hasPreviewPath } = normalizeCollectionsOption(marketingCollections);
  assert.deepEqual(hasPreviewPath, { blog: true, helpDocs: true });
});

test("marketingCollections blog previewPath resolves welcome slug", () => {
  const { previewPaths } = normalizeCollectionsOption(marketingCollections);
  assert.equal(previewPaths.get("blog")({ slug: "welcome" }), "/blog/welcome/");
});

test("marketing previewPath helpers match normalize output", () => {
  assert.equal(marketingPreviewPaths.blog({ slug: "welcome" }), "/blog/welcome/");
  assert.equal(marketingPreviewPaths.helpDocs({ slug: "getting-started" }), "/help/getting-started/");
});