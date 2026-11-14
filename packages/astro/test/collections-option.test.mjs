import assert from "node:assert/strict";
import { test } from "node:test";

import { normalizeCollectionsOption } from "../src/collections-option.js";

const stubCollection = { loader: {}, schema: () => null };

test.describe("normalizeCollectionsOption", () => {
  test("splits collection values from previewPath functions", () => {
    const previewPath = ({ slug }) => `/blog/${slug}/`;
    const input = {
      blog: { collection: stubCollection, previewPath },
      help: { collection: { loader: {}, schema: null } },
    };
    const { astroCollections, previewPaths, hasPreviewPath } = normalizeCollectionsOption(input);

    assert.deepEqual(astroCollections, {
      blog: stubCollection,
      help: { loader: {}, schema: null },
    });
    assert.equal(previewPaths.size, 1);
    assert.equal(previewPaths.get("blog"), previewPath);
    assert.deepEqual(hasPreviewPath, { blog: true, help: false });
  });

  test("sets hasPreviewPath false when previewPath is omitted", () => {
    const { hasPreviewPath, previewPaths } = normalizeCollectionsOption({
      pages: { collection: stubCollection },
    });
    assert.equal(hasPreviewPath.pages, false);
    assert.equal(previewPaths.size, 0);
  });

  test("throws when previewPath is not a function", () => {
    assert.throws(
      () => normalizeCollectionsOption({ blog: { collection: stubCollection, previewPath: "/blog/" } }),
      /previewPath must be a function/,
    );
  });

  test("throws when an entry is not { collection, previewPath? }", () => {
    assert.throws(
      () => normalizeCollectionsOption({ blog: stubCollection }),
      /must be \{ collection, previewPath/,
    );
  });
});