import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import { glob } from "astro/loaders";
import { GLOB_META, inferKind, isSupportedPattern, orsoGlob } from "orso-astro";

/**
 * Minimal loader context: enough for Astro's glob loader to sync one Markdown
 * file into a fake store, so pass-through options are observed as behaviour.
 */
async function syncLoader(loader) {
  const root = await mkdtemp(join(tmpdir(), "orso-astro-"));
  try {
    await mkdir(join(root, "content"), { recursive: true });
    await writeFile(join(root, "content", "hello.md"), "---\ntitle: Hello\n---\n\nBody\n", "utf8");
    const entries = new Map();
    const store = {
      keys: () => [...entries.keys()],
      get: (id) => entries.get(id),
      set: (entry) => {
        entries.set(entry.id, entry);
      },
      delete: (id) => {
        entries.delete(id);
      },
    };
    const entryType = {
      getEntryInfo: async ({ contents }) => ({ body: contents, data: { title: "Hello" } }),
      getRenderFunction: async () => async () => ({ html: "<p>Hello</p>" }),
    };
    await loader.load({
      collection: "blog",
      config: {
        root: pathToFileURL(`${root}/`),
        srcDir: pathToFileURL(`${root}/src/`),
        prerenderConflictBehavior: "warn",
      },
      logger: { info() {}, warn() {}, error() {}, debug() {} },
      watcher: null,
      parseData: async ({ data }) => data,
      generateDigest: (contents) => String(contents.length),
      store,
      meta: new Map(),
      entryTypes: new Map([[".md", entryType]]),
    });
    return [...entries.values()];
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test("GLOB_META is the shared orso.glob symbol", () => {
  assert.equal(GLOB_META, Symbol.for("orso.glob"));
});

test("orsoGlob attaches non-enumerable, frozen loader metadata", () => {
  const loader = orsoGlob({ base: "./src/content/blog/en", pattern: "*.md" });
  const descriptor = Object.getOwnPropertyDescriptor(loader, GLOB_META);
  assert.ok(descriptor, "loader should carry GLOB_META");
  assert.equal(descriptor.enumerable, false);
  assert.equal(Object.isFrozen(descriptor.value), true);
  assert.deepEqual(descriptor.value, {
    base: "src/content/blog/en",
    pattern: "*.md",
    kind: "markdown",
    singleton: null,
  });
});

test("orsoGlob keeps an explicit kind and singleton", () => {
  const loader = orsoGlob({
    base: "src/content/pages",
    pattern: "*.mdx",
    kind: "mdx",
    singleton: "home",
  });
  assert.deepEqual(loader[GLOB_META], {
    base: "src/content/pages",
    pattern: "*.mdx",
    kind: "mdx",
    singleton: "home",
  });
});

test("kind inference follows the pattern extension", () => {
  assert.equal(inferKind("*.md"), "markdown");
  assert.equal(inferKind("*.mdx"), "mdx");
  assert.equal(inferKind("*.json"), "json");
  assert.equal(inferKind("*.txt"), "unknown");
  assert.equal(isSupportedPattern("markdown", "*.md"), true);
  assert.equal(isSupportedPattern("unknown", "*.txt"), false);
});

test("orsoGlob passes generateId, retainBody and deferRender to Astro's glob loader", async () => {
  const loader = orsoGlob({
    base: "content",
    pattern: "*.md",
    generateId: ({ entry }) => `custom-${entry}`,
    retainBody: false,
    deferRender: true,
  });
  const [entry] = await syncLoader(loader);
  assert.ok(entry, "the glob loader should have stored one entry");
  assert.equal(entry.id, "custom-hello.md");
  assert.equal(entry.body, undefined);
  assert.equal(entry.deferredRender, true);
});

test("orsoGlob leaves Astro's glob defaults alone", async () => {
  const loader = orsoGlob({ base: "content", pattern: "*.md" });
  const [entry] = await syncLoader(loader);
  assert.ok(entry, "the glob loader should have stored one entry");
  assert.equal(entry.deferredRender, undefined);
  assert.equal(entry.rendered.html, "<p>Hello</p>");
  assert.equal(typeof entry.body, "string");
});

test("orsoGlob rejects invalid configuration", () => {
  assert.throws(() => orsoGlob({ base: "", pattern: "*.md" }), /base/);
  assert.throws(() => orsoGlob({ base: "src/content" }), /pattern/);
  assert.throws(() => orsoGlob({ base: "src/content", pattern: "*.md", singleton: true }), /singleton/);
});

test("plain Astro glob loaders carry no orso metadata", () => {
  const loader = glob({ base: "src/content", pattern: "*.md" });
  assert.equal(loader[GLOB_META], undefined);
});
