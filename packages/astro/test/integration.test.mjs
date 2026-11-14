import assert from "node:assert/strict";
import { test } from "node:test";
import { existsSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import { orso } from "orso-astro";

const noop = () => {};
const logger = { error: noop, warn: noop, info: noop };

test("astro:config:setup injects the preview bridge and checks panel during dev", () => {
  const injected = [];
  const { hooks } = orso();
  hooks["astro:config:setup"]({
    command: "dev",
    updateConfig: noop,
    logger,
    injectScript: (stage, content) => injected.push({ stage, content }),
  });
  assert.equal(injected.length, 2);
  assert.equal(injected[0].stage, "head-inline");
  assert.match(injected[0].content, /orso:preview/);
  // The frame is cross-origin, so the app learns the reader's page only from the bridge.
  assert.match(injected[0].content, /post\("path"/);
  assert.match(injected[0].content, /location\.href/);
  assert.equal(injected[1].stage, "page");
  assert.match(injected[1].content, /^import /);
  assert.match(injected[1].content, /panel\/panel\.js/);
});

test("astro:config:setup leaves builds alone", () => {
  const injected = [];
  const { hooks } = orso();
  hooks["astro:config:setup"]({
    command: "build",
    updateConfig: noop,
    logger,
    injectScript: (stage, content) => injected.push({ stage, content }),
  });
  assert.deepEqual(injected, []);
});

for (const command of ["build", "preview"]) {
  test(`astro:server:setup does nothing during \`astro ${command}\``, async () => {
    const root = await mkdtemp(join(tmpdir(), "orso-integration-"));
    try {
      const { hooks } = orso();
      hooks["astro:config:setup"]({ command, updateConfig: noop, logger });
      hooks["astro:config:done"]({ config: { root: pathToFileURL(`${root}/`), srcDir: pathToFileURL(join(root, "src/")) } });
      const used = [];
      const server = { environments: {}, middlewares: { use: (handler) => used.push(handler) } };

      await hooks["astro:server:setup"]({ server, logger });

      assert.deepEqual(used, []);
      assert.equal(existsSync(join(root, ".astro/orso-content.json")), false);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
}

async function distWithBrokenLink() {
  const dir = await mkdtemp(join(tmpdir(), "orso-build-done-"));
  await writeFile(
    join(dir, "index.html"),
    `<!DOCTYPE html><html lang="en"><head><title>Home</title><meta name="description" content="Welcome."></head><body><a href="/gone/">gone</a></body></html>`,
  );
  return dir;
}

async function distWarningsOnly() {
  const dir = await mkdtemp(join(tmpdir(), "orso-build-done-"));
  await writeFile(
    join(dir, "index.html"),
    `<!DOCTYPE html><html lang="en"><head><title>Home</title><meta name="description" content="Welcome home."></head><body><h1>Home</h1></body></html>`,
  );
  return dir;
}

test("astro:build:done prints report and fails when dist has errors", async () => {
  const dir = await distWithBrokenLink();
  const prevExit = process.exitCode;
  process.exitCode = undefined;
  try {
    const { hooks } = orso();
    hooks["astro:config:done"]({ config: { base: "/" } });
    const logs = [];
    const buildLogger = { ...logger, info: (msg) => logs.push(msg) };

    await assert.rejects(
      () =>
        hooks["astro:build:done"]({
          dir: pathToFileURL(dir),
          logger: buildLogger,
          pages: [],
          assets: new Map(),
        }),
      /Preview checks failed/,
    );

    assert.equal(process.exitCode, 1);
    assert.match(logs.join("\n"), /Preview checks/);
    assert.match(logs.join("\n"), /broken-link|ERROR/i);
  } finally {
    process.exitCode = prevExit;
    await rm(dir, { recursive: true, force: true });
  }
});

test("astro:build:done does not fail on warnings-only findings", async () => {
  const dir = await distWarningsOnly();
  const prevExit = process.exitCode;
  process.exitCode = undefined;
  try {
    const { hooks } = orso();
    hooks["astro:config:done"]({ config: { base: "/" } });

    await hooks["astro:build:done"]({
      dir: pathToFileURL(dir),
      logger,
      pages: [],
      assets: new Map(),
    });

    assert.notEqual(process.exitCode, 1);
  } finally {
    process.exitCode = prevExit;
    await rm(dir, { recursive: true, force: true });
  }
});
