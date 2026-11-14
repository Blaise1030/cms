const REGISTRY_KEY = "orso-collections";

/** @type {Record<string, unknown> | null} */
let astroCollections = null;

export function bindOrsoCollections(collections) {
  astroCollections = collections;
}

export function getOrsoCollections() {
  if (!astroCollections) {
    throw new Error(
      'orso: import "virtual:orso/collections" requires orso({ collections: { ... } }) in astro.config',
    );
  }
  return astroCollections;
}

/** @internal */
export function resetOrsoCollectionsForTests() {
  astroCollections = null;
}

export { REGISTRY_KEY };