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

test("astro:config:setup injects the preview bridge during dev", () => {
  const injected = [];
  const { hooks } = orso();
  hooks["astro:config:setup"]({
    command: "dev",
    updateConfig: noop,
    logger,
    injectScript: (stage, content) => injected.push({ stage, content }),
  });
  assert.equal(injected.length, 1);
  assert.equal(injected[0].stage, "head-inline");
  assert.match(injected[0].content, /orso:preview/);
  // The frame is cross-origin, so the app learns the reader's page only from the bridge.
  assert.match(injected[0].content, /post\("path"/);
  assert.match(injected[0].content, /location\.href/);
  // Page details snapshot for the CMS sidebar (no minified panel inject).
  assert.match(injected[0].content, /op === "snapshot"/);
});

test("astro:config:setup does not register the collections virtual plugin without collections", () => {
  const configs = [];
  const { hooks } = orso();
  hooks["astro:config:setup"]({
    command: "dev",
    updateConfig: (patch) => configs.push(patch),
    logger,
    injectScript: noop,
  });
  assert.equal(configs.length, 0);
});

test("astro:config:setup registers the collections virtual plugin when collections are provided", () => {
  const configs = [];
  const integration = orso({
    collections: {
      blog: { collection: { loader: {}, schema: () => null } },
    },
  });
  integration.hooks["astro:config:setup"]({
    command: "dev",
    updateConfig: (patch) => configs.push(patch),
    logger,
    injectScript: noop,
  });
  assert.equal(configs.length, 1);
  const plugin = configs[0].vite.plugins[0];
  assert.equal(plugin.name, "orso-virtual-collections");
});

/** The scripts `astro:config:setup` injects for `command`, with `ORSO_PREVIEW` set to `preview`. */
function injectedFor(command, preview) {
  const before = process.env.ORSO_PREVIEW;
  if (preview === undefined) delete process.env.ORSO_PREVIEW;
  else process.env.ORSO_PREVIEW = preview;
  try {
    const injected = [];
    orso().hooks["astro:config:setup"]({
      command,
      updateConfig: noop,
      logger,
      injectScript: (stage, content) => injected.push({ stage, content }),
    });
    return injected;
  } finally {
    if (before === undefined) delete process.env.ORSO_PREVIEW;
    else process.env.ORSO_PREVIEW = before;
  }
}

test("astro:config:setup leaves production builds alone", () => {
  for (const preview of [undefined, "", "0", "true"]) {
    assert.deepEqual(injectedFor("build", preview), [], `ORSO_PREVIEW=${preview}`);
  }
});

test("a pull request preview build (ORSO_PREVIEW=1) gets the bridge with self-capture", () => {
  const injected = injectedFor("build", "1");
  assert.equal(injected.length, 1);
  assert.equal(injected[0].stage, "head-inline");
  assert.match(injected[0].content, /__orsoCaptureRect/);
  assert.match(injected[0].content, /orso:preview/);
  // Wrapped, so neither part adds page globals.
  assert.match(injected[0].content, /^\(\(\) => \{/);
  // The dev bridge has no capture.
  assert.doesNotMatch(injectedFor("dev", "1")[0].content, /function __orsoCaptureRect/);
});

/**
 * Runs bridge `source` in a tiny fake window; returns what it posted to its parent and a way to
 * send it a message from `origin`.
 */
async function fakeWindow(source) {
  const vm = await import("node:vm");
  const posted = [];
  const listeners = {};
  // Copied out of the sandbox, so they compare like plain objects.
  const parent = { postMessage: (m) => posted.push(JSON.parse(JSON.stringify(m))) };
  const doc = { addEventListener: noop, removeEventListener: noop, documentElement: { style: {} } };
  const context = {
    addEventListener: (type, fn) => (listeners[type] = fn),
    removeEventListener: noop,
    document: doc,
    location: { href: "https://pr-3.site.test/pricing" },
    parent,
    URL,
  };
  context.window = context;
  vm.runInNewContext(source, context);
  const send = async (data, origin = "http://orso.localhost:7800") => {
    listeners.message({ source: parent, origin, data: { type: "orso:preview", ...data } });
    await new Promise((r) => setTimeout(r, 0));
  };
  return { posted, send };
}

test("the bridge answers the handshake and captures an area when it can", async () => {
  const { readFileSync } = await import("node:fs");
  const bridge = readFileSync(new URL("../src/bridge.js", import.meta.url), "utf8");
  const withCapture = `(() => {
    async function __orsoCaptureRect(rect) { return "data:image/png;base64," + rect.width + "x" + rect.height; }
    ${bridge}
  })();`;
  const page = await fakeWindow(withCapture);
  await page.send({ op: "hello" });
  assert.deepEqual(page.posted.at(-1), { type: "orso:preview", op: "ready", version: 1, capture: true });
  await page.send({ op: "capture", id: 7, rect: { x: 1, y: 2, width: 30, height: 40 } });
  assert.deepEqual(page.posted.at(-1), { type: "orso:preview", op: "capture", id: 7, ok: true, dataUrl: "data:image/png;base64,30x40" });
  // Only a loopback parent (orso itself) is answered.
  const before = page.posted.length;
  await page.send({ op: "hello" }, "https://evil.example");
  assert.equal(page.posted.length, before);

  // The dev bridge says it can't capture.
  const dev = await fakeWindow(bridge);
  await dev.send({ op: "hello" });
  assert.equal(dev.posted.at(-1).capture, false);
  await dev.send({ op: "capture", id: 1, rect: { x: 0, y: 0, width: 10, height: 10 } });
  assert.deepEqual(dev.posted.at(-1), {
    type: "orso:preview",
    op: "capture",
    id: 1,
    ok: false,
    error: "This preview can't capture itself.",
  });
});

test("a capture that fails says why", async () => {
  const { readFileSync } = await import("node:fs");
  const bridge = readFileSync(new URL("../src/bridge.js", import.meta.url), "utf8");
  const page = await fakeWindow(`(() => {
    async function __orsoCaptureRect() { throw new Error("The capture came out blank."); }
    ${bridge}
  })();`);
  await page.send({ op: "capture", id: 2, rect: { x: 0, y: 0, width: 5, height: 5 } });
  assert.deepEqual(page.posted.at(-1), {
    type: "orso:preview",
    op: "capture",
    id: 2,
    ok: false,
    error: "The capture came out blank.",
  });
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
