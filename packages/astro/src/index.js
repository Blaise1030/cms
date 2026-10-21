import { glob } from "astro/loaders";

import { GLOB_META, inferKind, isSupportedPattern, normalizeBase } from "./manifest.js";

export { GLOB_META, inferKind, isSupportedPattern, normalizeBase };

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

/**
 * Minimal Astro integration. Task 5 wires the manifest writer and the
 * dev-only content endpoint into its hooks.
 */
export function orso() {
  return { name: "orso", hooks: {} };
}
