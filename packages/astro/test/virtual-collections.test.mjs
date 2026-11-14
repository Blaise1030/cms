import assert from "node:assert/strict";
import { test } from "node:test";

import {
  RESOLVED_VIRTUAL_COLLECTIONS_ID,
  VIRTUAL_COLLECTIONS_ID,
  createVirtualCollectionsPlugin,
} from "../src/virtual-collections.js";

const stubCollection = { loader: { type: "stub" }, schema: () => null };

test("virtual module resolves and exports astroCollections", async () => {
  const blog = { ...stubCollection };
  const help = { loader: { type: "help" }, schema: null };
  const plugin = createVirtualCollectionsPlugin({ astroCollections: { blog, help } });

  assert.equal(plugin.resolveId(VIRTUAL_COLLECTIONS_ID), RESOLVED_VIRTUAL_COLLECTIONS_ID);
  assert.equal(plugin.resolveId("./other.js"), undefined);

  const code = plugin.load(RESOLVED_VIRTUAL_COLLECTIONS_ID);
  assert.match(code, /export const collections = getOrsoCollections\(\)/);

  const { getOrsoCollections, resetOrsoCollectionsForTests } = await import("../src/collections-runtime-registry.js");
  try {
    assert.equal(getOrsoCollections().blog.loader.type, "stub");
    assert.equal(getOrsoCollections().help.loader.type, "help");
    assert.equal("previewPath" in getOrsoCollections().blog, false);
  } finally {
    resetOrsoCollectionsForTests();
  }
});

test("misconfigured import throws a clear orso error", () => {
  const plugin = createVirtualCollectionsPlugin({ astroCollections: null });
  assert.throws(
    () => plugin.load(RESOLVED_VIRTUAL_COLLECTIONS_ID),
    /import ["']virtual:orso\/collections["'] requires orso\(\{ collections/,
  );
});