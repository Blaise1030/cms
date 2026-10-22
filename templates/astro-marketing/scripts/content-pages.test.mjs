import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const html = (dir, path) => readFileSync(join(dir, "dist", path, "index.html"), "utf8");
function fixture(run) {
  const dir = mkdtempSync(join(tmpdir(), "content-pages-"));
  try {
    for (const path of ["src", "astro.config.mjs", "package.json"]) cpSync(join(root, path), join(dir, path), { recursive: true });
    symlinkSync(join(root, "node_modules"), join(dir, "node_modules"), "dir");
    run(dir);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}
function build(dir, base = "/preview/example/") {
  const result = spawnSync(process.execPath, [join(root, "node_modules/astro/bin/astro.mjs"), "build"], {
    cwd: dir, encoding: "utf8", env: { ...process.env, PUBLIC_BASE: base },
  });
  assert.equal(result.status, 0, result.stdout + result.stderr);
}

test("built sample Blog and Help articles render Markdown and navigation", () => {
  assert.match(html(root, "blog"), /Welcome to the blog/);
  assert.match(html(root, "blog/welcome"), /<h2[^>]*>.*Welcome/);
  assert.match(html(root, "help"), /Getting started/);
  assert.match(html(root, "help/getting-started"), /<h2[^>]*>.*First steps/);
  assert.match(html(root, ""), /href="\/blog\/"[^>]*>Blog/);
  assert.match(html(root, ""), /href="\/help\/"[^>]*>Help/);
  for (const id of ["hero", "logos", "features", "testimonials", "pricing", "faq", "cta"]) assert.match(html(root, ""), new RegExp(`id="${id}"`));
});

// Fixtures vary filenames and dates/orders to catch sorting by filesystem order.
test("base-prefixed routes sort Blog and Help entries", () => fixture((dir) => {
  for (const name of ["blog", "helpDocs"]) mkdirSync(join(dir, "src/content", name, "en"), { recursive: true });
  writeFileSync(join(dir, "src/content/blog/en/a-new.md"), '---\ntitle: Newest article\ndescription: New article\npubDate: "2026-10-11"\n---\nNew **body**');
  writeFileSync(join(dir, "src/content/blog/en/z-old.md"), '---\ntitle: Oldest article\ndescription: Old article\npubDate: "2020-01-01"\n---\nOld body');
  for (const [slug, title, section, order] of [["a-last", "Last guide", "Setup", 9], ["z-first", "First guide", "Setup", 1], ["b-tie", "Alpha guide", "Setup", 9], ["c-other", "Other guide", "Reference", 1]]) {
    writeFileSync(join(dir, `src/content/helpDocs/en/${slug}.md`), `---\ntitle: ${title}\ndescription: Guide\nsection: ${section}\norder: ${order}\n---\nGuide body`);
  }
  build(dir);
  const blog = html(dir, "blog");
  assert.ok(blog.indexOf("Newest article") < blog.indexOf("Oldest article"));
  assert.match(blog, /href="\/preview\/example\/blog\/a-new\/"/);
  assert.match(blog, /href="\/preview\/example\/#features"/);
  const help = html(dir, "help");
  assert.ok(help.indexOf("First guide") < help.indexOf("Alpha guide"));
  assert.ok(help.indexOf("Alpha guide") < help.indexOf("Last guide"));
  assert.ok(help.indexOf("Reference") < help.indexOf("Other guide"));
  assert.match(help, /href="\/preview\/example\/help\/z-first\/"/);
  assert.match(html(dir, "blog/a-new"), /<strong>body<\/strong>/);
}));

test("empty collections retain useful Blog and Help indexes", () => fixture((dir) => {
  for (const name of ["blog", "helpDocs"]) {
    rmSync(join(dir, "src/content", name), { recursive: true, force: true });
    mkdirSync(join(dir, "src/content", name, "en"), { recursive: true });
  }
  build(dir);
  assert.match(html(dir, "blog"), /No blog posts yet/);
  assert.match(html(dir, "help"), /No help docs yet/);
}));