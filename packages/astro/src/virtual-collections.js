import { bindOrsoCollections } from "./collections-runtime-registry.js";

export const VIRTUAL_COLLECTIONS_ID = "virtual:orso/collections";
export const RESOLVED_VIRTUAL_COLLECTIONS_ID = `\0${VIRTUAL_COLLECTIONS_ID}`;

const REGISTRY_MODULE = new URL("./collections-runtime-registry.js", import.meta.url).href;

const MISCONFIG_MESSAGE =
  'orso: import "virtual:orso/collections" requires orso({ collections: { ... } }) in astro.config';

/**
 * Vite plugin that materializes {@link VIRTUAL_COLLECTIONS_ID} from normalized
 * Astro collection definitions (previewPath functions stay out of the bundle).
 */
export function createVirtualCollectionsPlugin({ astroCollections } = {}) {
  if (astroCollections) {
    bindOrsoCollections(astroCollections);
  }

  return {
    name: "orso-virtual-collections",
    resolveId(id) {
      if (id === VIRTUAL_COLLECTIONS_ID) return RESOLVED_VIRTUAL_COLLECTIONS_ID;
    },
    load(id) {
      if (id !== RESOLVED_VIRTUAL_COLLECTIONS_ID) return;
      if (!astroCollections) {
        throw new Error(MISCONFIG_MESSAGE);
      }
      return `import { getOrsoCollections } from ${JSON.stringify(REGISTRY_MODULE)};\nexport const collections = getOrsoCollections();\n`;
    },
  };
}