import assert from "node:assert/strict";
import { test } from "node:test";
import { existsSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
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
