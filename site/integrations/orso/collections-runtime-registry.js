const REGISTRY_KEY = Symbol.for("orso.astro.collections");

/** @returns {Record<string, unknown> | null} */
function readRegistry() {
  return globalThis[REGISTRY_KEY] ?? null;
}

export function bindOrsoCollections(collections) {
  globalThis[REGISTRY_KEY] = collections;
}

export function getOrsoCollections() {
  const astroCollections = readRegistry();
  if (!astroCollections) {
    throw new Error(
      'orso: import "virtual:orso/collections" requires orso({ collections: { ... } }) in astro.config',
    );
  }
  return astroCollections;
}

/** @internal */
export function resetOrsoCollectionsForTests() {
  delete globalThis[REGISTRY_KEY];
}

export { REGISTRY_KEY };