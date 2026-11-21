import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const html = (dir, path) => readFileSync(join(dir, "dist", path, "index.html"), "utf8");
function fixture(run) {
  const dir = mkdtempSync(join(tmpdir(), "content-pages-"));
  try {
    for (const path of ["src", "public", "integrations", "astro.config.mjs", "package.json"]) cpSync(join(root, path), join(dir, path), { recursive: true });
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
  assert.match(html(root, ""), /href="\/blog\/"[^>]*>(?:<[^>]+>)*Blog/);
  assert.match(html(root, ""), /href="\/help\/"[^>]*>(?:<[^>]+>)*Help/);
  for (const id of ["hero", "logos", "features", "testimonials", "pricing", "faq", "cta"]) assert.match(html(root, ""), new RegExp(`id="${id}"`));
});

test("built sample blog categories link their posts", () => {
  assert.match(html(root, "blog"), /href="\/blog\/categories\/guides\/"[^>]*>(?:<[^>]+>)*Guides/);
  assert.match(html(root, "blog/categories"), /<h1[^>]*>Blog categories/);
  const guides = html(root, "blog/categories/guides");
  assert.match(guides, /<h1[^>]*>Guides/);
  assert.match(guides, /href="\/blog\/plan-a-launch-in-a-week\/"/);
  assert.doesNotMatch(guides, /href="\/blog\/welcome\/"/);
  assert.match(html(root, "blog/welcome"), /Keep reading/);
  // The search palette island points at its section's index.
  assert.match(html(root, "blog"), /bundlePath&quot;:\[0,&quot;\/pagefind-blog\/&quot;\]/);
  assert.ok(existsSync(join(root, "dist", "pagefind-blog", "pagefind.js")), "pagefind-blog index");
});

// Fixtures vary filenames and dates/orders to catch sorting by filesystem order.
test("base-prefixed routes sort Blog and Help entries", () => fixture((dir) => {
  for (const name of ["blog", "helpDocs", "helpCategories"]) mkdirSync(join(dir, "src/content", name, "en"), { recursive: true });
  writeFileSync(join(dir, "src/content/helpCategories/en/reference.md"), "---\ntitle: Reference\ndescription: Look things up\norder: 1\n---\n");
  writeFileSync(join(dir, "src/content/blog/en/a-new.md"), '---\ntitle: Newest article\ndescription: New article\npubDate: "2026-10-11"\n---\nNew **body**');
  writeFileSync(join(dir, "src/content/blog/en/z-old.md"), '---\ntitle: Oldest article\ndescription: Old article\npubDate: "2020-01-01"\n---\nOld body');
  for (const [slug, title, category, order] of [["a-last", "Last guide", "Setup", 9], ["z-first", "First guide", "Setup", 1], ["b-tie", "Alpha guide", "Setup", 9], ["c-other", "Other guide", "reference", 1]]) {
    writeFileSync(join(dir, `src/content/helpDocs/en/${slug}.md`), `---\ntitle: ${title}\ndescription: Guide\ncategory: ${category}\norder: ${order}\n---\nGuide body\n\n## One\n\nText\n\n## Two\n\nText`);
  }
  build(dir);
  const blog = html(dir, "blog");
  assert.ok(blog.indexOf("Newest article") < blog.indexOf("Oldest article"));
  assert.match(blog, /href="\/preview\/example\/blog\/a-new\/"/);
  assert.match(blog, /href="\/preview\/example\/#features"/);
  const help = html(dir, "help");
  assert.ok(help.indexOf("First guide") < help.indexOf("Alpha guide"));
  assert.ok(help.indexOf("Alpha guide") < help.indexOf("Last guide"));
  // A category file sorts before free-text categories.
  assert.ok(help.indexOf("Reference") < help.indexOf("Other guide"));
  assert.ok(help.indexOf("Other guide") < help.indexOf("First guide"));
  assert.match(help, /bundlePath&quot;:\[0,&quot;\/preview\/example\/pagefind-help\/&quot;\]/);
  assert.ok(existsSync(join(dir, "dist", "pagefind-help", "pagefind.js")), "pagefind-help index");
  const reference = html(dir, "help/category/reference");
  assert.match(reference, /<h1[^>]*>Reference/);
  assert.match(reference, /href="\/preview\/example\/help\/c-other\/"/);
  assert.match(html(dir, "help/z-first"), /On this page/);
  assert.match(help, /href="\/preview\/example\/help\/z-first\/"/);
  assert.match(html(dir, "blog/a-new"), /<strong>body<\/strong>/);
  // Free-text categories (no category file) still get a category page.
  assert.match(help, /href="\/preview\/example\/help\/category\/setup\/"/);
  assert.match(html(dir, "help/category/setup"), /<h1[^>]*>Setup/);
  // SEO files are base-aware and list the collections.
  const file = (path) => readFileSync(join(dir, "dist", path), "utf8");
  assert.match(file("robots.txt"), /^Sitemap: https:\/\/example\.com\/preview\/example\/sitemap\.xml$/m);
  assert.match(file("sitemap.xml"), /<loc>https:\/\/example\.com\/preview\/example\/blog\/a-new\/<\/loc>/);
  assert.match(file("sitemap.xml"), /<loc>https:\/\/example\.com\/preview\/example\/help\/z-first\/<\/loc>/);
  assert.match(file("sitemap.xml"), /<loc>https:\/\/example\.com\/preview\/example\/privacy\/<\/loc>/);
  assert.match(file("llms.txt"), /^# Orso\n\n> /);
  assert.match(file("llms.txt"), /- \[Newest article\]\(https:\/\/example\.com\/preview\/example\/blog\/a-new\/\): New article/);
  assert.match(html(dir, "blog/a-new"), /<link rel="canonical" href="https:\/\/example\.com\/preview\/example\/blog\/a-new\/"/);
  assert.match(html(dir, "blog/a-new"), /"@type":"BlogPosting"/);
}));

test("empty or missing collections retain useful Blog and Help indexes", () => fixture((dir) => {
  // Sites made before the category collections existed have no folders for them at all.
  for (const name of ["blogCategories", "helpCategories"]) {
    rmSync(join(dir, "src/content", name), { recursive: true, force: true });
  }
  for (const name of ["blog", "helpDocs"]) {
    rmSync(join(dir, "src/content", name), { recursive: true, force: true });
    mkdirSync(join(dir, "src/content", name, "en"), { recursive: true });
  }
  build(dir);
  assert.match(html(dir, "blog"), /No blog posts yet/);
  assert.match(html(dir, "help"), /No help docs yet/);
  assert.match(html(dir, "blog/categories"), /No blog categories yet/);
  assert.match(readFileSync(join(dir, "dist/sitemap.xml"), "utf8"), /<loc>https:\/\/example\.com\/preview\/example\/<\/loc>/);
}));