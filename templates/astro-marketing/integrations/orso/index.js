import { createRequire } from "node:module";
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const getAstroGlobLoader = () => require("astro/loaders").glob;

import { auditDist } from "./checks/audit.js";
import { exitCode, formatReport } from "./checks/report.js";
import { normalizeCollectionsOption } from "./collections-option.js";
import { createEndpoint } from "./endpoint.js";
import { createVirtualCollectionsPlugin } from "./virtual-collections.js";
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
  const loader = getAstroGlobLoader()({ base: normalizedBase, pattern: normalizedPattern, ...options });
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

/**
 * The bridge between orso's app and the (always cross-origin) preview frame: picking,
 * appearance, address, errors, Page details. Injected inline during `astro dev` and into
 * pull request preview builds; never into a production build.
 */
const PREVIEW_BRIDGE = readFileSync(new URL("./bridge.js", import.meta.url), "utf8");
/** Self-capture for picked elements and review screenshots. */
const PREVIEW_CAPTURE = readFileSync(new URL("./capture.js", import.meta.url), "utf8");

/**
 * The bridge with self-capture, in one function so neither adds globals to the page.
 * `appOrigins` are the only origins it talks to (empty: any loopback parent that says hello).
 */
export function previewBridge(appOrigins = []) {
  const origins = `const __orsoAppOrigins = ${JSON.stringify(appOrigins)};`;
  return `(() => {\n${origins}\n${PREVIEW_CAPTURE}\n${PREVIEW_BRIDGE}\n})();`;
}

/** The bridge for a `ORSO_PREVIEW=1` build, which knows no app origins. */
export const PREVIEW_BUILD_BRIDGE = previewBridge();

/** `ORSO_APP_ORIGINS`: the comma-separated app origins the daemon lets the bridge talk to. */
function appOriginsFromEnv(value = process.env.ORSO_APP_ORIGINS) {
  return (value ?? "")
    .split(",")
    .map((origin) => origin.trim())
    .filter((origin) => /^https?:\/\/[^/\s]+$/i.test(origin));
}

/**
 * Vite settings for running under orso's dev server. The daemon serves the preview at the root
 * of its own origin (`ORSO_PREVIEW_PORT`) and tunnels the HMR socket there, so the browser must
 * open HMR on that port rather than the dev server's own. Nothing otherwise: sites set no base
 * and no HMR options for orso.
 */
export function orsoDevViteConfig(previewPort = process.env.ORSO_PREVIEW_PORT) {
  const port = Number(previewPort);
  if (!Number.isInteger(port) || port <= 0 || port > 65535) return null;
  return { server: { hmr: { clientPort: port } } };
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
async function setup({ config, server, logger, live, hasPreviewPath, previewPaths, hide = [] }) {
  const root = fileURLToPath(config.root);
  const configPath = findContentConfig(fileURLToPath(config.srcDir));
  const runner = runnerOf(server);
  let manifest = { version: 1, collections: [] };
  let schemas = new Map();

  const load = async () => {
    try {
      const result = await loadContent({
        runner,
        configPath,
        ...(hasPreviewPath ? { hasPreviewPath } : {}),
        hide,
      });
      for (const name of result.unknownHidden) {
        logger.warn(`orso({ hide }) names "${name}", which is not a content collection; ignoring it.`);
      }
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
    server.middlewares.use(
      createEndpoint({ token, lookup, previewPaths: previewPaths instanceof Map ? previewPaths : new Map() }),
    );
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

/** `orso({ hide })`: an optional array of collection names. */
function normalizeHideOption(input) {
  if (input === undefined) return [];
  if (!Array.isArray(input) || input.some((name) => typeof name !== "string" || name.trim() === "")) {
    throw new TypeError('orso: "hide" must be an array of collection names');
  }
  return [...new Set(input.map((name) => name.trim()))];
}

/**
 * The Orso Astro integration.
 *
 * `astro dev` and `astro sync` write `.astro/orso-content.json`. The dev server
 * also reloads it when the content config changes and mounts `POST /__orso`
 * when `ORSO_CONTENT_TOKEN` is set. On `astro build`, runs preview checks over the
 * output directory, and injects the preview bridge only when `ORSO_PREVIEW=1` (a pull
 * request's preview deploy).
 *
 * `hide` lists collections orso leaves out on purpose (for example translated copies
 * another tool manages). The editor skips them without the warning an unsupported
 * collection gets.
 */
export function orso(options = {}) {
  let config;
  let command;
  const collectionsOption = options.collections;
  const hide = normalizeHideOption(options.hide);
  let astroCollections = null;
  let previewPaths = null;
  let hasPreviewPath = null;

  if (collectionsOption !== undefined) {
    ({ astroCollections, previewPaths, hasPreviewPath } = normalizeCollectionsOption(collectionsOption));
  }

  return {
    name: "orso-content",
    hooks: {
      // `astro sync` runs as production, where Astro skips `astro:server:setup`.
      "astro:config:setup": ({ command: current, updateConfig, logger, injectScript }) => {
        command = current;
        // orso frames this server on an origin of its own, so the bridge is the app's only
        // way into the page.
        if (command === "dev") {
          injectScript("head-inline", previewBridge(appOriginsFromEnv()));
        } else if (command === "build" && process.env.ORSO_PREVIEW === "1") {
          // A pull request's preview deploy: orso frames it in reviews and screenshots areas
          // of it without asking to share the screen. Never in production builds.
          injectScript("head-inline", PREVIEW_BUILD_BRIDGE);
        }

        const vitePlugins = [];
        if (astroCollections) {
          vitePlugins.push(createVirtualCollectionsPlugin({ astroCollections }));
        }
        if (command === "sync") {
          vitePlugins.push({
            name: "orso-content-sync",
            configureServer: (server) =>
              setup({
                config,
                server,
                logger,
                live: false,
                hasPreviewPath: hasPreviewPath ?? undefined,
                previewPaths: previewPaths ?? undefined,
                hide,
              }),
          });
        }
        const vite = {};
        if (vitePlugins.length > 0) vite.plugins = vitePlugins;
        const underOrso = command === "dev" ? orsoDevViteConfig() : null;
        if (underOrso) Object.assign(vite, underOrso);
        if (Object.keys(vite).length > 0) {
          updateConfig({ vite });
        }
      },
      "astro:config:done": ({ config: done }) => {
        config = done;
      },
      "astro:server:setup": async ({ server, logger }) => {
        // `build --devOutput` and `astro sync` outside production also start a server.
        if (command !== "dev") return;
        await setup({
          config,
          server,
          logger,
          live: true,
          hasPreviewPath: hasPreviewPath ?? undefined,
          previewPaths: previewPaths ?? undefined,
          hide,
        });
      },
      "astro:build:done": async ({ dir, logger }) => {
        const distPath = fileURLToPath(dir);
        const base = config?.base;
        const opts = typeof base === "string" && base !== "" ? { base } : {};
        const { findings } = await auditDist(distPath, opts);
        logger.info(formatReport(findings));
        if (exitCode(findings) === 1) {
          process.exitCode = 1;
          throw new Error("Preview checks failed.");
        }
      },
    },
  };
}
