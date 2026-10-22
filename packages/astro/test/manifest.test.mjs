import assert from "node:assert/strict";
import { mkdtemp, readdir, readFile, rm, writeFile, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { glob } from "astro/loaders";
import { z } from "astro/zod";
import {
  blocked,
  describeSchema,
  isSupportedPattern,
  loadContent,
  orsoGlob,
  writeManifest,
} from "orso-astro";

import { createCollectionSchemas } from "./fixtures/schemas.mjs";

const CONFIG = "/virtual/src/content.config.ts";

/** The subset of Astro's `defineCollection` shape the manifest needs. */
function fakeRunner(module) {
  const calls = [];
  return {
    calls,
    import: async (path) => {
      calls.push(path);
      return module;
    },
  };
}

function blogCollection(overrides = {}) {
  return {
    loader: orsoGlob({ base: "./src/content/blog/en", pattern: "*.md", ...overrides }),
    schema: ({ image }) => createCollectionSchemas({ z, image }).blog,
  };
}

const helpDocsCollection = {
  loader: orsoGlob({ base: "./src/content/helpDocs/en", pattern: "*.md" }),
  schema: createCollectionSchemas({ z }).helpDocs,
};

async function loadCollections(collections, extra = {}) {
  const runner = fakeRunner({ collections });
  const result = await loadContent({ runner, configPath: CONFIG });
  return { ...result, runner, ...extra };
}

test.describe("describeSchema", () => {
  test("describes primitive leaves", () => {
    assert.deepEqual(describeSchema(z.string()), { type: "string", required: true });
    assert.deepEqual(describeSchema(z.number()), { type: "number", required: true });
    assert.deepEqual(describeSchema(z.boolean()), { type: "boolean", required: true });
    assert.deepEqual(describeSchema(z.date()), { type: "date", required: true });
  });

  test("uses editor metadata for date and known refinements", () => {
    const date = z.string()
      .regex(/^\d{4}-\d{2}-\d{2}$/)
      .refine((value) => value.length === 10)
      .meta({ editor: { type: "date", knownRefinement: true } });
    assert.deepEqual(describeSchema(date), { type: "date", required: true });

    const refined = z.object({ title: z.string() })
      .superRefine(() => {})
      .meta({ editor: { knownRefinement: true } });
    assert.deepEqual(describeSchema(refined), {
      type: "object",
      required: true,
      properties: { title: { type: "string", required: true } },
    });
  });

  test("marks optional and defaulted fields as not required", () => {
    assert.deepEqual(describeSchema(z.string().optional()), { type: "string", required: false });
    assert.deepEqual(describeSchema(z.array(z.string()).default([])), {
      type: "array",
      required: false,
      default: [],
      items: { type: "string", required: true },
    });
  });

  test("uses editor metadata for image and reference fields", () => {
    assert.deepEqual(
      describeSchema(z.string().meta({ editor: { type: "image" } }).optional()),
      { type: "image", required: false },
    );
    assert.deepEqual(
      describeSchema(z.string().meta({ editor: { type: "reference", collection: "blog" } })),
      { type: "reference", collection: "blog", required: true },
    );
  });

  test("describes arrays and objects recursively", () => {
    assert.deepEqual(describeSchema(z.array(z.number())), {
      type: "array",
      required: true,
      items: { type: "number", required: true },
    });
    assert.deepEqual(describeSchema(z.object({ title: z.string(), order: z.number().optional() })), {
      type: "object",
      required: true,
      properties: {
        title: { type: "string", required: true },
        order: { type: "number", required: false },
      },
    });
  });

  test("describes literals, enums and discriminated unions", () => {
    assert.deepEqual(describeSchema(z.literal("home")), {
      type: "literal",
      required: true,
      values: ["home"],
    });
    assert.deepEqual(describeSchema(z.enum(["a", "b"])), {
      type: "enum",
      required: true,
      values: ["a", "b"],
    });
    assert.deepEqual(describeSchema(z.discriminatedUnion("kind", [
      z.object({ kind: z.literal("article"), title: z.string() }),
      z.object({ kind: z.literal("note"), body: z.string().optional() }),
    ])), {
      type: "union",
      required: true,
      discriminator: "kind",
      variants: [
        {
          type: "object",
          required: true,
          properties: {
            kind: { type: "literal", required: true, values: ["article"] },
            title: { type: "string", required: true },
          },
        },
        {
          type: "object",
          required: true,
          properties: {
            kind: { type: "literal", required: true, values: ["note"] },
            body: { type: "string", required: false },
          },
        },
      ],
    });
  });

  test("reports transforms, unknown refinements and plain unions as unsupported", () => {
    assert.deepEqual(describeSchema(z.string().transform((value) => value.length)), {
      type: "unsupported",
      required: true,
      reason: "Unknown transform cannot be represented",
    });
    assert.deepEqual(describeSchema(z.string().pipe(z.string())), {
      type: "unsupported",
      required: true,
      reason: "Unknown transform cannot be represented",
    });
    assert.deepEqual(describeSchema(z.object({ title: z.string() }).refine(() => true)), {
      type: "unsupported",
      required: true,
      reason: "Unknown custom refinement cannot be represented",
    });
    assert.deepEqual(describeSchema(z.union([z.string(), z.number()])), {
      type: "unsupported",
      required: true,
      reason: "Non-discriminated union cannot be represented",
    });
  });
});

test.describe("blocked", () => {
  test("is true for a required unsupported leaf", () => {
    assert.equal(blocked(describeSchema(z.string().transform((value) => value))), true);
    assert.equal(blocked(describeSchema(z.object({ nested: z.string().transform((value) => value) }))), true);
    assert.equal(blocked(describeSchema(z.array(z.string().transform((value) => value)))), true);
    assert.equal(blocked(describeSchema(z.discriminatedUnion("kind", [
      z.object({ kind: z.literal("a"), value: z.string().transform((value) => value) }),
      z.object({ kind: z.literal("b") }),
    ]))), true);
  });

  test("is false when the unsupported leaf is optional", () => {
    assert.equal(blocked(describeSchema(z.string().transform((value) => value).optional())), false);
    assert.equal(blocked(describeSchema(z.object({
      nested: z.string().transform((value) => value).optional(),
    }))), false);
    assert.equal(blocked(describeSchema(z.object({ title: z.string() }))), false);
  });
});

test.describe("isSupportedPattern", () => {
  test("matches the Rust loader rules", () => {
    assert.equal(isSupportedPattern("markdown", "*.md"), true);
    assert.equal(isSupportedPattern("mdx", "*.mdx"), true);
    assert.equal(isSupportedPattern("json", "*.json"), true);
    assert.equal(isSupportedPattern("json", "home.json"), true);
  });

  test("rejects nested patterns, mismatched kinds and other extensions", () => {
    assert.equal(isSupportedPattern("markdown", "**/*.md"), false);
    assert.equal(isSupportedPattern("markdown", "*.mdx"), false);
    assert.equal(isSupportedPattern("mdx", "*.md"), false);
    assert.equal(isSupportedPattern("json", "data/*.json"), false);
    assert.equal(isSupportedPattern("json", "*.jsonc"), false);
    assert.equal(isSupportedPattern("unknown", "*.md"), false);
  });
});

test.describe("loadContent", () => {
  test("builds the manifest and schema map from orsoGlob collections", async () => {
    const { manifest, schemas, runner } = await loadCollections({
      blog: blogCollection(),
      helpDocs: helpDocsCollection,
    });

    assert.deepEqual(runner.calls, [CONFIG]);
    assert.equal(manifest.version, 1);
    assert.deepEqual(manifest.collections.map(({ name }) => name), ["blog", "helpDocs"]);
    assert.deepEqual(manifest.collections.map(({ loader }) => loader), [
      { base: "src/content/blog/en", pattern: "*.md" },
      { base: "src/content/helpDocs/en", pattern: "*.md" },
    ]);
    assert.deepEqual(manifest.collections.map(({ kind }) => kind), ["markdown", "markdown"]);
    assert.deepEqual(manifest.collections.map(({ singleton }) => singleton), [null, null]);
    assert.deepEqual(manifest.collections.map(({ supportedForEditing }) => supportedForEditing), [true, true]);
    assert.deepEqual(manifest.collections.map(({ reason }) => reason), [null, null]);

    assert.ok(schemas instanceof Map);
    assert.equal(schemas.size, 2);
    assert.equal(schemas.get("blog")._def.type, "object");
    assert.equal(schemas.get("helpDocs")._def.type, "object");
  });

  test("describes the blog fields exactly as the template schema declares them", async () => {
    const { manifest } = await loadCollections({ blog: blogCollection(), helpDocs: helpDocsCollection });
    const fields = manifest.collections[0].fields;

    assert.equal(fields.type, "object");
    assert.equal(fields.required, true);
    assert.deepEqual(fields.properties.title, { type: "string", required: true });
    assert.deepEqual(fields.properties.description, { type: "string", required: true });
    assert.deepEqual(fields.properties.pubDate, { type: "date", required: true });
    assert.deepEqual(fields.properties.coverImage, { type: "image", required: false });
    assert.deepEqual(fields.properties.coverAlt, { type: "string", required: false });
    assert.deepEqual(fields.properties.tags, {
      type: "array",
      required: false,
      default: [],
      items: { type: "string", required: true },
    });
    assert.deepEqual(fields.properties.relatedPosts, {
      type: "array",
      required: false,
      default: [],
      items: { type: "reference", collection: "blog", required: true },
    });

    assert.deepEqual(manifest.collections[1].fields.properties, {
      title: { type: "string", required: true },
      description: { type: "string", required: true },
      section: { type: "string", required: true },
      order: { type: "number", required: true },
    });
  });

  test("keeps an explicit singleton", async () => {
    const { manifest, schemas } = await loadCollections({
      home: {
        loader: orsoGlob({ base: "src/content/pages", pattern: "*.mdx", kind: "mdx", singleton: "home" }),
        schema: z.object({ title: z.string() }),
      },
    });
    assert.deepEqual(manifest.collections[0], {
      name: "home",
      loader: { base: "src/content/pages", pattern: "*.mdx" },
      kind: "mdx",
      singleton: "home",
      supportedForEditing: true,
      reason: null,
      fields: {
        type: "object",
        required: true,
        properties: { title: { type: "string", required: true } },
      },
    });
    assert.equal(schemas.size, 1);
  });

  test("lists a plain Astro glob loader with loader null and read-only", async () => {
    const { manifest } = await loadCollections({
      pages: {
        loader: glob({ base: "src/content/pages", pattern: "*.md" }),
        schema: z.object({ title: z.string() }),
      },
    });
    assert.deepEqual(manifest.collections[0], {
      name: "pages",
      loader: null,
      kind: "unknown",
      singleton: null,
      supportedForEditing: false,
      reason: "Collection does not use orsoGlob",
      fields: {
        type: "object",
        required: true,
        properties: { title: { type: "string", required: true } },
      },
    });
  });

  test("marks an unsafe base read-only", async () => {
    const { manifest } = await loadCollections({
      outside: {
        loader: orsoGlob({ base: "../outside", pattern: "*.md" }),
        schema: z.object({ title: z.string() }),
      },
    });
    assert.equal(manifest.collections[0].loader.base, "../outside");
    assert.equal(manifest.collections[0].supportedForEditing, false);
    assert.equal(manifest.collections[0].reason, "Loader base must stay inside the project");
  });

  test("marks an unsupported pattern read-only", async () => {
    const { manifest } = await loadCollections({
      nested: {
        loader: orsoGlob({ base: "src/content", pattern: "**/*.md" }),
        schema: z.object({ title: z.string() }),
      },
    });
    assert.equal(manifest.collections[0].kind, "markdown");
    assert.equal(manifest.collections[0].supportedForEditing, false);
    assert.equal(manifest.collections[0].reason, "Loader pattern is not supported for editing");
  });

  test("marks a collection without a schema read-only", async () => {
    const { manifest, schemas } = await loadCollections({
      noSchema: { loader: orsoGlob({ base: "src/content/pages", pattern: "*.md" }) },
    });
    assert.equal(manifest.collections[0].fields, null);
    assert.equal(manifest.collections[0].supportedForEditing, false);
    assert.equal(manifest.collections[0].reason, "Collection has no schema");
    assert.equal(schemas.size, 0);
  });

  test("marks a schema with required unsupported fields read-only", async () => {
    const { manifest } = await loadCollections({
      blocked: {
        loader: orsoGlob({ base: "src/content/blocked", pattern: "*.md" }),
        schema: z.object({ special: z.string().transform((value) => value) }),
      },
    });
    assert.equal(manifest.collections[0].supportedForEditing, false);
    assert.equal(manifest.collections[0].reason, "Schema has required unsupported fields");
    assert.equal(manifest.collections[0].fields.properties.special.type, "unsupported");
  });

  test("one broken schema does not hide the other collections", async () => {
    const { manifest, schemas } = await loadCollections({
      broken: {
        loader: orsoGlob({ base: "src/content/broken", pattern: "*.md" }),
        schema: () => {
          throw new Error("boom");
        },
      },
      blog: blogCollection(),
    });
    assert.deepEqual(manifest.collections.map(({ name }) => name), ["broken", "blog"]);
    assert.equal(manifest.collections[0].fields, null);
    assert.equal(manifest.collections[0].supportedForEditing, false);
    assert.equal(manifest.collections[0].reason, "Schema could not be inspected: boom");
    assert.equal(manifest.collections[1].supportedForEditing, true);
    assert.equal(schemas.size, 1);
    assert.equal(schemas.has("blog"), true);
  });

  test("treats a missing configPath as an empty manifest", async () => {
    const runner = { import: () => assert.fail("a missing config must not be imported") };
    assert.deepEqual(await loadContent({ runner }), {
      manifest: { version: 1, collections: [] },
      schemas: new Map(),
    });
    assert.deepEqual(await loadContent(), {
      manifest: { version: 1, collections: [] },
      schemas: new Map(),
    });
  });

  test("rethrows a whole-module import failure", async () => {
    const runner = {
      import: async () => {
        throw new Error("cannot import config");
      },
    };
    await assert.rejects(loadContent({ runner, configPath: CONFIG }), /cannot import config/);
  });

  test("rejects a config whose collections is not an object", async () => {
    for (const collections of [undefined, null, [], "blog", 42]) {
      await assert.rejects(
        loadContent({ runner: fakeRunner({ collections }), configPath: CONFIG }),
        /collections/,
      );
    }
  });
});

test.describe("writeManifest", () => {
  test("writes pretty JSON with a trailing newline under .astro", async () => {
    const root = await mkdtemp(join(tmpdir(), "orso-manifest-"));
    try {
      const manifest = {
        version: 1,
        collections: [{ name: "blog", loader: { base: "src/content/blog", pattern: "*.md" } }],
      };
      writeManifest(root, manifest);
      const text = await readFile(join(root, ".astro", "orso-content.json"), "utf8");
      assert.equal(text, `${JSON.stringify(manifest, null, 2)}\n`);
      assert.deepEqual(JSON.parse(text), manifest);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("replaces an existing manifest without leaving temporary files", async () => {
    const root = await mkdtemp(join(tmpdir(), "orso-manifest-"));
    try {
      await mkdir(join(root, ".astro"), { recursive: true });
      await writeFile(join(root, ".astro", "orso-content.json"), '{"version":1,"collections":[{"name":"stale"}]}\n');

      const manifest = { version: 1, collections: [{ name: "blog" }] };
      writeManifest(root, manifest);
      writeManifest(root, manifest);

      const directory = join(root, ".astro");
      assert.deepEqual(await readdir(directory), ["orso-content.json"]);
      assert.deepEqual(JSON.parse(await readFile(join(directory, "orso-content.json"), "utf8")), manifest);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});
