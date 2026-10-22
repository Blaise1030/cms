import { randomUUID } from "node:crypto";
import { mkdirSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { z } from "astro/zod";

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

/**
 * Whether the editor can patch a collection with this kind and pattern.
 *
 * Mirrors the Rust catalog's rules: Markdown and MDX collections must use the
 * flat `*.md`/`*.mdx` pattern, and JSON may use `*.json` or name a single
 * `*.json` filename. Nested patterns such as `**\/*.md` are read-only because
 * the editor cannot tell which files an edit would affect.
 */
export function isSupportedPattern(kind, pattern) {
  if (typeof kind !== "string" || typeof pattern !== "string") return false;
  if (kind === "json") {
    return pattern === "*.json" || (!pattern.includes("*") && pattern.endsWith(".json"));
  }
  if (kind === "markdown") return pattern === "*.md";
  if (kind === "mdx") return pattern === "*.mdx";
  return false;
}

/**
 * A loader base is safe to read and write only while it stays inside the
 * project: relative, non-empty, NUL-free and without a `..` component.
 */
function isSafeBase(base) {
  if (typeof base !== "string" || base === "" || base.includes("\0")) return false;
  if (base.startsWith("/") || base.startsWith("\\") || /^[A-Za-z]:[\\/]/.test(base)) return false;
  return !base.split(/[\\/]/).includes("..");
}

/** A limited projection of live Zod schemas, never a validation schema. */
export function describeSchema(schema) {
  const def = schema._def, metadata = schema.meta?.()?.editor ?? {};
  if (def.type === "optional" || def.type === "default") return { ...describeSchema(def.innerType), required: false, ...(def.type === "default" ? { default: def.defaultValue } : {}) };
  const base = { required: true };
  const unsupported = (reason) => ({ ...base, type: "unsupported", reason });
  if (def.type === "pipe" || def.type === "transform") return unsupported("Unknown transform cannot be represented");
  if (def.checks?.some((check) => check._zod?.def?.check === "custom") && !metadata.knownRefinement) return unsupported("Unknown custom refinement cannot be represented");
  if (metadata.type) return { ...base, type: metadata.type, ...(metadata.collection ? { collection: metadata.collection } : {}) };
  switch (def.type) {
    case "string": case "boolean": case "number": case "date": return { ...base, type: def.type };
    case "literal": return { ...base, type: "literal", values: def.values };
    case "enum": return { ...base, type: "enum", values: Object.values(def.entries) };
    case "array": return { ...base, type: "array", items: describeSchema(def.element) };
    case "object": return { ...base, type: "object", properties: Object.fromEntries(Object.entries(def.shape).map(([name, field]) => [name, describeSchema(field)])) };
    case "union":
      if (def.discriminator) return { ...base, type: "union", discriminator: def.discriminator, variants: def.options.map(describeSchema) };
      return unsupported("Non-discriminated union cannot be represented");
    default: return unsupported(`Unsupported schema node: ${def.type}`);
  }
}

/** True when the schema requires a field the editor cannot represent. */
export function blocked(field) {
  if (field.required === false) return false;
  if (field.type === "unsupported") return true;
  return Object.values(field.properties ?? {}).some(blocked) || (field.items ? blocked(field.items) : false) || (field.variants ?? []).some(blocked);
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * One manifest entry for a collection. A schema-function failure is recorded
 * on that collection only, so one broken schema never hides the others.
 */
async function describeCollection(name, definition, schemas) {
  const meta = definition?.loader?.[GLOB_META] ?? null;
  const entry = {
    name,
    loader: meta ? { base: meta.base, pattern: meta.pattern } : null,
    kind: typeof meta?.kind === "string" ? meta.kind : "unknown",
    singleton: typeof meta?.singleton === "string" ? meta.singleton : null,
    supportedForEditing: false,
    reason: null,
    fields: null,
  };

  let schema = null;
  let schemaError = null;
  try {
    const declared = definition?.schema;
    schema = typeof declared === "function" ? await declared({ image: () => z.string() }) : declared;
    if (schema != null) {
      entry.fields = describeSchema(schema);
      schemas.set(name, schema);
    }
  } catch (error) {
    schemaError = error;
    entry.fields = null;
  }

  if (!meta) entry.reason = "Collection does not use orsoGlob";
  else if (!isSafeBase(meta.base)) entry.reason = "Loader base must stay inside the project";
  else if (!isSupportedPattern(entry.kind, meta.pattern)) entry.reason = "Loader pattern is not supported for editing";
  else if (schemaError) entry.reason = `Schema could not be inspected: ${errorMessage(schemaError)}`;
  else if (!schema) entry.reason = "Collection has no schema";
  else if (blocked(entry.fields)) entry.reason = "Schema has required unsupported fields";
  else entry.supportedForEditing = true;
  return entry;
}

/**
 * Import a content config and build the manifest plus the live schema map.
 *
 * A missing `configPath` is an empty manifest (projects without collections).
 * A module that cannot be imported, or one that does not export a `collections`
 * object, throws so the caller can keep the previous manifest.
 */
export async function loadContent({ runner, configPath } = {}) {
  const schemas = new Map();
  if (!configPath) return { manifest: { version: 1, collections: [] }, schemas };

  const module = await runner.import(configPath);
  const collections = module?.collections;
  if (!collections || typeof collections !== "object" || Array.isArray(collections)) {
    throw new TypeError('orso: content config must export a "collections" object');
  }

  const entries = [];
  for (const [name, definition] of Object.entries(collections)) {
    entries.push(await describeCollection(name, definition, schemas));
  }
  return { manifest: { version: 1, collections: entries }, schemas };
}

/**
 * Write the manifest atomically: a unique temporary file in the same
 * directory, then a rename over `.astro/orso-content.json`.
 */
export function writeManifest(root, manifest) {
  const directory = join(root, ".astro");
  mkdirSync(directory, { recursive: true });
  const target = join(directory, "orso-content.json");
  const temporary = join(directory, `.orso-content.${process.pid}.${randomUUID()}.tmp`);
  try {
    writeFileSync(temporary, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
    renameSync(temporary, target);
  } catch (error) {
    try {
      unlinkSync(temporary);
    } catch {
      // The temporary file may never have been created.
    }
    throw error;
  }
}
