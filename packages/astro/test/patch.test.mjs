import assert from "node:assert/strict";
import { test } from "node:test";

import { z } from "astro/zod";
import jsyaml from "js-yaml";
import { PatchError, SchemaIssuesError, patchSource } from "orso-astro";

import { createCollectionSchemas } from "./fixtures/schemas.mjs";

const schemas = createCollectionSchemas({ z });
const blog = (request) => patchSource(request, { kind: "markdown", schema: schemas.blog });

const pageSchema = z.object({
  title: z.string(),
  description: z.string().optional(),
  tags: z.array(z.string()).default([]),
  menu: z.object({ label: z.string(), href: z.string() }).optional(),
});
const page = (request) => patchSource(request, { kind: "json", schema: pageSchema });

test("patchSource changes YAML values while retaining quotes, comments, CRLF and body bytes", () => {
  const source = "---\r\n# heading\r\ntitle: 'Old' # keep\r\ndescription: \"Unchanged\"\r\npubDate: 2026-10-11\r\ntags: [one, two] # tags\r\n---\r\n\r\nBody  \r\n";
  assert.equal(blog({ source, fields: { title: "New" } }), source.replace("'Old'", "'New'"));
  assert.equal(blog({ source, fields: { title: "Old" } }), source);
  assert.equal(blog({ source, fields: {}, body: "Changed\n" }), source.slice(0, source.indexOf("\r\nBody")) + "Changed\n");
});

test("patchSource validates the complete merged candidate before returning source", () => {
  const source = "---\ntitle: Old\ndescription: Kept\npubDate: 2026-10-11\n---\nBody";

  assert.throws(() => blog({ source, fields: { pubDate: "2026-02-30" } }), (error) => {
    assert.ok(error instanceof SchemaIssuesError);
    assert.deepEqual(error.issues, [{ path: ["pubDate"], message: "Use a valid calendar date" }]);
    assert.match(error.message, /pubDate/);
    return true;
  });

  // The schema's superRefine runs on the merged candidate, not on the patch alone.
  assert.throws(() => blog({ source, fields: { coverImage: "./cover.png" } }), (error) => {
    assert.ok(error instanceof SchemaIssuesError);
    assert.deepEqual(error.issues.map(({ path, message }) => ({ path, message })), [
      { path: ["coverAlt"], message: "Describe the cover image" },
    ]);
    return true;
  });
});

test("patchSource inserts and edits YAML while preserving untouched nodes", () => {
  const source = "---\ntitle: \"Hello\" # retain\ndescription: 'Old' # label\npubDate: 2026-10-11 # retain\n---\n<Widget />\n";
  assert.equal(blog({ source, fields: { description: "New" } }), source.replace("'Old'", "'New'"));
  const inserted = blog({ source, fields: { coverAlt: "Added" } });
  assert.ok(inserted.includes(source.slice(0, source.indexOf("---\n<Widget"))));
  assert.ok(inserted.endsWith("---\n<Widget />\n"));
});

test("patchSource retains the following field when a block scalar changes to multiline text", () => {
  const source = "---\ntitle: Old\ndescription: |\n  Old line\n  Other line\npubDate: 2026-10-11 # keep\n---\nBody";
  const result = blog({ source, fields: { description: "New\nText" } });
  assert.ok(result.includes("pubDate: 2026-10-11 # keep\n---\nBody"));
  assert.deepEqual(jsyaml.load(result.split("---\n")[1]).description, "New\nText");
});

for (const [name, frontmatter, fields] of [
  ["block scalar header comment", "description: | # preserve this comment\n  Old line\n", { description: "New" }],
  ["a commented sequence item", "description: Kept\ntags:\n  - one # preserve one\n  - two # preserve two\n", { tags: ["two"] }],
]) {
  test(`patchSource safely rejects replacing ${name}`, () => {
    const source = "---\ntitle: Post\npubDate: 2026-10-11\n" + frontmatter + "---\nBody";
    assert.throws(
      () => blog({ source, fields }),
      (error) => error instanceof PatchError && /cannot safely.*comment/i.test(error.message),
    );
  });
}

test("patchSource appending to a YAML list keeps the comments on existing items", () => {
  const source = "---\ntitle: Post\npubDate: 2026-10-11\ndescription: Kept\ntags:\n  - one # preserve one\n  - two # preserve two\n---\nBody";
  assert.equal(
    blog({ source, fields: { tags: ["one", "two", "three"] } }),
    source.replace("two # preserve two\n", "two # preserve two\n  - three\n"),
  );
});

test("patchSource unset removes optional top-level fields while preserving surrounding source", () => {
  const source = "---\r\ntitle: 'Post' # keep\r\ndescription: Kept\r\npubDate: 2026-10-11\r\ncoverImage: ./cover.png\r\ncoverAlt: Cover\r\ntags: [one] # tags\r\n---\r\nBody\r\n";
  assert.equal(
    blog({ source, fields: {}, unset: ["coverImage", "coverAlt"] }),
    source.replace("coverImage: ./cover.png\r\ncoverAlt: Cover\r\n", ""),
  );
  assert.equal(blog({ source, fields: {}, unset: ["missing"] }), source);
});

test("patchSource unset validates the candidate and rejects malformed or overlapping names", () => {
  const source = "---\ntitle: Post\ndescription: Kept\npubDate: 2026-10-11\n---\nBody";
  assert.throws(() => blog({ source, fields: {}, unset: ["title"] }), (error) => {
    assert.ok(error instanceof SchemaIssuesError);
    assert.match(error.message, /title/);
    return true;
  });
  for (const request of [{ unset: "title" }, { unset: [["title"]] }, { unset: ["title"], fields: { title: "New" } }]) {
    assert.throws(
      () => blog({ source, fields: {}, ...request }),
      (error) => error instanceof PatchError && /unset/.test(error.message),
    );
  }
  assert.throws(
    () => blog({ source, fields: "title" }),
    (error) => error instanceof PatchError && /fields must be an object/.test(error.message),
  );
});

test("patchSource gains and loses YAML block and flow list items in their own style", () => {
  const block = "---\ntitle: Post\ndescription: D\npubDate: \"2026-10-11\"\ntags:\n  - news\n  - launch\nrelatedPosts: []\n---\nBody";
  assert.equal(blog({ source: block, fields: { tags: ["news", "launch", "x"] } }), block.replace("  - launch\n", "  - launch\n  - x\n"));
  assert.equal(blog({ source: block, fields: { tags: ["launch"] } }), block.replace("  - news\n", ""));
  assert.equal(blog({ source: block, fields: { tags: [] } }), block.replace("tags:\n  - news\n  - launch\n", "tags: []\n"));
  assert.equal(blog({ source: block, fields: { relatedPosts: ["welcome"] } }), block.replace("relatedPosts: []", "relatedPosts: [welcome]"));

  const flow = "---\ntitle: Post\ndescription: D\npubDate: \"2026-10-11\"\ntags: [news]\n---\nBody";
  assert.equal(blog({ source: flow, fields: { tags: ["news", "launch"] } }), flow.replace("[news]", "[news, launch]"));
  assert.equal(blog({ source: flow, fields: { tags: [] } }), flow.replace("[news]", "[]"));
});

test("patchSource refuses YAML lists whose comments cannot be preserved with a Code hint", () => {
  const source = "---\ntitle: Post\ndescription: D\npubDate: \"2026-10-11\"\ntags:\n  - news # first\n  - launch\n---\nBody";
  assert.throws(
    () => blog({ source, fields: { tags: ["launch"] } }),
    (error) => error instanceof PatchError && /edit this field in Code/.test(error.message),
  );
});

test("patchSource writes date-like strings so Astro frontmatter parsing keeps them strings", () => {
  const source = "---\ntitle: Post\ndescription: D\npubDate: \"2026-10-11\"\ntags: []\n---\nBody";
  for (const fields of [{ title: "2026-10-12" }, { title: "true" }, { tags: ["2026-10-12"] }, { coverAlt: "2026-10-12" }]) {
    const result = blog({ source, fields });
    const parsed = jsyaml.load(result.split("---\n")[1]);
    for (const [key, value] of Object.entries(fields)) assert.deepEqual(parsed[key], value, result);
  }
  const block = "---\ntitle: Post\ndescription: D\npubDate: \"2026-10-11\"\ntags:\n  - news\n---\nBody";
  const parsed = jsyaml.load(blog({ source: block, fields: { tags: ["news", "2026-10-12"] } }).split("---\n")[1]);
  assert.deepEqual(parsed.tags, ["news", "2026-10-12"]);
});

test("patchSource preserves YAML anchors and aliases when an unrelated field changes", () => {
  const source = "---\ntitle: &t Anchor\ndescription: *t\npubDate: 2026-10-11\ncoverAlt: Cover\n\n---\nBody\n";
  assert.equal(blog({ source, fields: { coverAlt: "New" } }), source.replace("coverAlt: Cover", "coverAlt: New"));
});

test("patchSource refuses an edit that would break a YAML anchor instead of dropping it", () => {
  const source = "---\ntitle: &t Anchor\ndescription: *t\npubDate: 2026-10-11\n---\nBody\n";
  assert.throws(
    () => blog({ source, fields: { title: "Changed" } }),
    (error) => error instanceof PatchError && error.message === "Patch could not preserve YAML structure",
  );
});

test("patchSource preserves blank lines in frontmatter", () => {
  const source = "---\ntitle: Post\ndescription: Kept\n\npubDate: 2026-10-11\ncoverAlt: Cover\n---\nBody\n";
  assert.equal(blog({ source, fields: { coverAlt: "New" } }), source.replace("coverAlt: Cover", "coverAlt: New"));
});

test("patchSource handles MDX frontmatter and body replacement", () => {
  const source = "---\ntitle: Old\ndescription: Kept\npubDate: 2026-10-11\n---\n\n<Widget value={1} />\n";
  assert.equal(
    patchSource({ source, fields: { title: "New" } }, { kind: "mdx", schema: schemas.blog }),
    source.replace("title: Old", "title: New"),
  );
  assert.equal(
    patchSource({ source, fields: {}, body: "" }, { kind: "mdx", schema: schemas.blog }),
    "---\ntitle: Old\ndescription: Kept\npubDate: 2026-10-11\n---\n",
  );
});

test("patchSource rejects missing frontmatter, non-string bodies and unknown kinds with PatchError", () => {
  const source = "---\ntitle: Old\npubDate: 2026-10-11\n---\nBody";
  assert.throws(
    () => blog({ source: "no frontmatter", fields: {} }),
    (error) => error instanceof PatchError && /Missing YAML frontmatter/.test(error.message),
  );
  assert.throws(
    () => blog({ source, fields: {}, body: 42 }),
    (error) => error instanceof PatchError && /body must be a string/.test(error.message),
  );
  assert.throws(
    () => patchSource({ source, fields: {} }, { kind: "yaml", schema: schemas.blog }),
    (error) => error instanceof PatchError && /kind/.test(error.message),
  );
});

test("patchSource keeps JSON key order, indentation and quoting for untouched bytes", () => {
  const source = '{\n  "title": "Old",\n  "description": "Kept",\n  "tags": ["one", "two"]\n}\n';
  assert.equal(page({ source, fields: { title: "New" } }), source.replace('"Old"', '"New"'));
  assert.equal(page({ source, fields: { title: "Old" } }), source);
  assert.equal(page({ source, fields: {}, body: "ignored\n" }), source);
});

test("patchSource preserves JSON comments while editing other fields", () => {
  const source = '{\n  // page metadata\n  "title": "Old",\n  /* keep */ "description": "Kept",\n  "tags": ["one"]\n}\n';
  assert.equal(page({ source, fields: { title: "New" } }), source.replace('"Old"', '"New"'));
  // The comment on the edited line also survives.
  const trailing = '{\n  "title": "Old", // heading\n  "description": "Kept"\n}\n';
  assert.equal(page({ source: trailing, fields: { title: "New" } }), trailing.replace('"Old"', '"New"'));
  // Inserting a key keeps the leading comment and formats the new line.
  const commented = '{\n  // page metadata\n  "title": "Page"\n}\n';
  assert.equal(
    page({ source: commented, fields: { description: "Added" } }),
    '{\n  // page metadata\n  "title": "Page",\n  "description": "Added"\n}\n',
  );
});

test("patchSource edits nested JSON leaves and inserts missing keys with the source indentation", () => {
  const nested = '{\n  "title": "Page",\n  "menu": { "label": "Home", "href": "/" }\n}\n';
  assert.equal(
    page({ source: nested, fields: { menu: { label: "Start", href: "/" } } }),
    nested.replace('"Home"', '"Start"'),
  );

  const source = '{\n  "title": "Page"\n}\n';
  assert.equal(
    page({ source, fields: { description: "Added" } }),
    '{\n  "title": "Page",\n  "description": "Added"\n}\n',
  );
  assert.equal(
    page({ source: '{\n\t"title": "Page"\n}\n', fields: { description: "Added" } }),
    '{\n\t"title": "Page",\n\t"description": "Added"\n}\n',
  );
  assert.equal(
    page({ source: '{\r\n  "title": "Page"\r\n}\r\n', fields: { description: "Added" } }),
    '{\r\n  "title": "Page",\r\n  "description": "Added"\r\n}\r\n',
  );

  // Inserting into an inline object expands it with the surrounding indentation.
  const menu = '{\n  "title": "Page",\n  "menu": { "label": "Home" }\n}\n';
  assert.equal(
    page({ source: menu, fields: { menu: { label: "Home", href: "/" } } }),
    '{\n  "title": "Page",\n  "menu": {\n    "label": "Home",\n    "href": "/"\n  }\n}\n',
  );
});

test("patchSource splices JSON lists in place using their own separators", () => {
  const inline = '{\n  "title": "Page",\n  "tags": ["one", "two"]\n}\n';
  assert.equal(
    page({ source: inline, fields: { tags: ["one", "two", "three"] } }),
    inline.replace('["one", "two"]', '["one", "two", "three"]'),
  );

  const block = '{\n  "title": "Page",\n  "tags": [\n    "one",\n    "two"\n  ]\n}\n';
  assert.equal(
    page({ source: block, fields: { tags: ["one", "two", "three"] } }),
    block.replace('    "two"\n', '    "two",\n    "three"\n'),
  );
  assert.equal(page({ source: block, fields: { tags: ["two"] } }), block.replace('    "one",\n', ""));
  assert.equal(page({ source: block, fields: { tags: [] } }), block.replace('"tags": [\n    "one",\n    "two"\n  ]', '"tags": []'));
});

test("patchSource refuses invalid or non-object JSON with PatchError", () => {
  for (const source of ["{", "[]", "null", '"text"']) {
    assert.throws(
      () => page({ source, fields: {} }),
      (error) => error instanceof PatchError,
      `expected PatchError for ${source}`,
    );
  }
});
