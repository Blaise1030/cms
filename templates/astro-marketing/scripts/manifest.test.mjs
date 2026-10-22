import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");

test("astro sync writes the orso content manifest", () => {
  const dir = mkdtempSync(join(tmpdir(), "orso-manifest-"));
  try {
    for (const path of ["src", "astro.config.mjs", "package.json"]) cpSync(join(root, path), join(dir, path), { recursive: true });
    symlinkSync(join(root, "node_modules"), join(dir, "node_modules"), "dir");
    const result = spawnSync(process.execPath, [join(root, "node_modules/astro/bin/astro.mjs"), "sync"], {
      cwd: dir, encoding: "utf8", env: { ...process.env, PUBLIC_BASE: "/" },
    });
    assert.equal(result.status, 0, result.stdout + result.stderr);

    const path = join(dir, ".astro/orso-content.json");
    assert.ok(existsSync(path), `missing ${path}\n${result.stdout}${result.stderr}`);
    const manifest = JSON.parse(readFileSync(path, "utf8"));
    assert.equal(manifest.version, 1);
    assert.deepEqual(manifest.collections.map(({ name }) => name), ["blog", "helpDocs"]);
    const byName = Object.fromEntries(manifest.collections.map((collection) => [collection.name, collection]));
    assert.deepEqual(byName.blog.loader, { base: "src/content/blog/en", pattern: "*.md" });
    assert.deepEqual(byName.helpDocs.loader, { base: "src/content/helpDocs/en", pattern: "*.md" });
    for (const collection of manifest.collections) assert.equal(collection.supportedForEditing, true, collection.reason);
    assert.equal(byName.blog.fields.properties.pubDate.type, "date");
    assert.equal(byName.blog.fields.properties.coverImage.type, "image");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
