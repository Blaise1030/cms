import assert from "node:assert/strict";
import { test } from "node:test";

import { normalizeCollectionsOption } from "../src/collections-option.js";
import { attachMarketingPreviewPaths, marketingCollections, marketingPreviewPaths } from "../src/marketing.js";

test("marketingCollections lists the blog and help collections in sidebar order", () => {
  assert.deepEqual(Object.keys(marketingCollections), [
    "blog",
    "blogCategories",
    "helpDocs",
    "helpCategories",
  ]);
});

test("marketingCollections sets hasPreviewPath for every routed collection via normalize", () => {
  const { hasPreviewPath } = normalizeCollectionsOption(marketingCollections);
  assert.deepEqual(hasPreviewPath, {
    blog: true,
    blogCategories: true,
    helpDocs: true,
    helpCategories: true,
  });
});

test("marketingCollections blog previewPath resolves welcome slug", () => {
  const { previewPaths } = normalizeCollectionsOption(marketingCollections);
  assert.equal(previewPaths.get("blog")({ slug: "welcome" }), "/blog/welcome/");
});

test("marketing previewPath helpers match normalize output", () => {
  assert.equal(marketingPreviewPaths.blog({ slug: "welcome" }), "/blog/welcome/");
  assert.equal(marketingPreviewPaths.helpDocs({ slug: "getting-started" }), "/help/getting-started/");
  assert.equal(marketingPreviewPaths.blogCategories({ slug: "guides" }), "/blog/categories/guides/");
  assert.equal(marketingPreviewPaths.helpCategories({ slug: "basics" }), "/help/category/basics/");
});

test("attachMarketingPreviewPaths keeps the two-collection call shape working", () => {
  const blog = { type: "content_layer" };
  const helpDocs = { type: "content_layer" };
  const wrapped = attachMarketingPreviewPaths({ blog, helpDocs });
  assert.deepEqual(Object.keys(wrapped), ["blog", "helpDocs"]);
  assert.equal(wrapped.blog.collection, blog);
  assert.equal(wrapped.helpDocs.previewPath({ slug: "x" }), "/help/x/");
});

test("attachMarketingPreviewPaths passes unknown collections through without a previewPath", () => {
  const custom = { type: "content_layer" };
  const wrapped = attachMarketingPreviewPaths({ custom });
  assert.deepEqual(wrapped, { custom: { collection: custom } });
  assert.deepEqual(normalizeCollectionsOption(wrapped).hasPreviewPath, { custom: false });
});
