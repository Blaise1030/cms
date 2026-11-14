/**
 * Split `orso({ collections })` entries into Astro collection definitions,
 * runtime previewPath handlers, and manifest-safe flags.
 */
export function normalizeCollectionsOption(input) {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw new TypeError('orso: "collections" must be an object');
  }

  const astroCollections = {};
  const previewPaths = new Map();
  const hasPreviewPath = {};

  for (const [name, entry] of Object.entries(input)) {
    if (!entry || typeof entry !== "object" || Array.isArray(entry) || !("collection" in entry)) {
      throw new TypeError(`orso: collections.${name} must be { collection, previewPath? }`);
    }
    astroCollections[name] = entry.collection;

    if (entry.previewPath !== undefined) {
      if (typeof entry.previewPath !== "function") {
        throw new TypeError(`orso: collections.${name}.previewPath must be a function`);
      }
      previewPaths.set(name, entry.previewPath);
      hasPreviewPath[name] = true;
    } else {
      hasPreviewPath[name] = false;
    }
  }

  return { astroCollections, previewPaths, hasPreviewPath };
}