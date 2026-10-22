import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { glob } from "astro/loaders";

import { createEndpoint } from "./endpoint.js";
import {
  GLOB_META,
  blocked,
  describeSchema,
  inferKind,
  isSupportedPattern,
  loadContent,
  normalizeBase,
  writeManifest,
} from "./manifest.js";

export {
  GLOB_META,
  blocked,
  describeSchema,
  inferKind,
  isSupportedPattern,
  loadContent,
  normalizeBase,
  writeManifest,
};

export { PatchError, SchemaIssuesError, patchSource } from "./patch.js";
export { createEndpoint } from "./endpoint.js";

/**
 * Wrap Astro's `glob()` loader and record the options Astro hides.
 *
 * The metadata lives under `Symbol.for("orso.glob")` as a non-enumerable,
 * frozen value so it never leaks into a content collection's schema or a
 * serialized loader. `generateId`, `retainBody` and `deferRender` (and any
 * other `glob()` option) are passed through unchanged.
 */
export function orsoGlob({ base, pattern, kind, singleton = null, ...options } = {}) {
  const normalizedBase = normalizeBase(base);
  if (typeof pattern !== "string" || pattern.trim() === "") {
    throw new Error('orsoGlob: "pattern" must be a non-empty string');
  }
  const normalizedPattern = pattern.trim();
  if (singleton !== null && singleton !== undefined && (typeof singleton !== "string" || singleton.trim() === "")) {
    throw new Error('orsoGlob: "singleton" must be a non-empty string or null');
  }
  const loader = glob({ base: normalizedBase, pattern: normalizedPattern, ...options });
  Object.defineProperty(loader, GLOB_META, {
    value: Object.freeze({
      base: normalizedBase,
      pattern: normalizedPattern,
      kind: typeof kind === "string" && kind.trim() !== "" ? kind : inferKind(normalizedPattern),
      singleton: singleton ?? null,
    }),
    enumerable: false,
    writable: false,
    configurable: false,
  });
  return loader;
}

const CONTENT_CONFIG_NAMES = ["content.config.ts", "content.config.js", "content.config.mjs", "content.config.mts"];

function findContentConfig(srcDir) {
  for (const name of CONTENT_CONFIG_NAMES) {
    const path = join(srcDir, name);
    if (existsSync(path)) return path;
  }
  return null;
}

function runnerOf(server) {
  const environment = server.environments?.astro ?? server.environments?.ssr;
  if (!environment?.runner) {
    throw new Error('orso: the Vite server has no runnable "astro" or "ssr" environment');
  }
  return environment.runner;
}

/**
 * Load the content config through the server's runner and write the manifest.
 * With `live`, also mount the content endpoint and reload on config changes.
 */
async function setup({ config, server, logger, live }) {
  const root = fileURLToPath(config.root);
  const configPath = findContentConfig(fileURLToPath(config.srcDir));
  const runner = runnerOf(server);
  let manifest = { version: 1, collections: [] };
  let schemas = new Map();

  const load = async () => {
    try {
      const result = await loadContent({ runner, configPath });
      writeManifest(root, result.manifest);
      ({ manifest, schemas } = result);
    } catch (error) {
      logger.error(`Could not load the content config; keeping the previous manifest. ${error?.stack ?? error}`);
    }
  };
  let pending = load();
  await pending;
  if (!live) return;

  const token = process.env.ORSO_CONTENT_TOKEN;
  if (token) {
    const lookup = (name) => {
      const entry = manifest.collections.find((collection) => collection.name === name);
      if (!entry) return undefined;
      return { supportedForEditing: entry.supportedForEditing, kind: entry.kind, reason: entry.reason, schema: schemas.get(name) };
    };
    server.middlewares.use(createEndpoint({ token, lookup }));
  }

  if (configPath && server.watcher) {
    server.watcher.add(configPath);
    server.watcher.on("change", (file) => {
      if (resolve(file) !== configPath) return;
      pending = pending.then(() => {
        for (const module of runner.evaluatedModules.getModulesByFile(configPath) ?? []) {
          runner.evaluatedModules.invalidateModule(module);
        }
        return load();
      });
    });
  }
}

/**
 * The Orso Astro integration.
 *
 * `astro dev` and `astro sync` write `.astro/orso-content.json`. The dev server
 * also reloads it when the content config changes and mounts `POST /__orso`
 * when `ORSO_CONTENT_TOKEN` is set. It does nothing during `astro build`.
 */
export function orso() {
  let config;
  let command;
  return {
    name: "orso-content",
    hooks: {
      // `astro sync` runs as production, where Astro skips `astro:server:setup`.
      "astro:config:setup": ({ command: current, updateConfig, logger }) => {
        command = current;
        if (command !== "sync") return;
        updateConfig({
          vite: {
            plugins: [{ name: "orso-content-sync", configureServer: (server) => setup({ config, server, logger, live: false }) }],
          },
        });
      },
      "astro:config:done": ({ config: done }) => {
        config = done;
      },
      "astro:server:setup": async ({ server, logger }) => {
        if (command === "sync") return;
        await setup({ config, server, logger, live: true });
      },
    },
  };
}
