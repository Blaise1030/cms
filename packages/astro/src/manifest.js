/**
 * Shared metadata for orso-wrapped Astro loaders.
 *
 * The symbol is registered globally so a loader created by one copy of the
 * package is still recognised by another (the template ships its own copy).
 */
export const GLOB_META = Symbol.for("orso.glob");

/** Content kinds the editor can read and write, keyed by file extension. */
const KIND_BY_EXTENSION = new Map([
  [".md", "markdown"],
  [".mdx", "mdx"],
  [".json", "json"],
]);

/**
 * Normalize a loader base to a site-root-relative, forward-slash path.
 * Leading `./` and trailing slashes are removed; the path is not resolved,
 * so `..` and absolute bases stay visible to the manifest (read-only there).
 */
export function normalizeBase(base) {
  if (typeof base !== "string" || base.trim() === "") {
    throw new Error('orsoGlob: "base" must be a non-empty string');
  }
  const normalized = base
    .trim()
    .replace(/\\/g, "/")
    .replace(/\/{2,}/g, "/")
    .replace(/^(?:\.\/)+/, "")
    .replace(/\/+$/, "");
  if (normalized === "") {
    throw new Error('orsoGlob: "base" must be a non-empty string');
  }
  return normalized;
}

/** Infer the content kind from a glob pattern's file extension. */
export function inferKind(pattern) {
  if (typeof pattern !== "string") return "unknown";
  const name = pattern.split("/").at(-1) ?? "";
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return "unknown";
  return KIND_BY_EXTENSION.get(name.slice(dot).toLowerCase()) ?? "unknown";
}

/** A pattern is editable when every file it matches has one known kind. */
export function isSupportedPattern(pattern) {
  return inferKind(pattern) !== "unknown";
}
