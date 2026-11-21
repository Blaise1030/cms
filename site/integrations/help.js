import { createRequire } from "node:module";

import { orsoGlob } from "./orso/index.js";

const require = createRequire(import.meta.url);
const getZod = () => require("astro/zod").z;

/**
 * Orso's own help centre: articles (`src/content/helpDocs/en/*.md`) grouped into categories
 * (`src/content/helpCategories/en/*.md`). Hand-rolled instead of the marketing preset's four
 * help collections, so there is one way to group an article. Kept out of `integrations/orso/`,
 * which Orso may overwrite.
 */

/** A help article, served at `/help/{id}/`. `category` names the category it is listed under. */
const helpDocs = {
  type: "content_layer",
  loader: orsoGlob({ base: "./src/content/helpDocs/en", pattern: "*.md" }),
  schema: () => {
    const z = getZod();
    return z.object({
      title: z.string(),
      description: z.string(),
      category: z.string().meta({ editor: { type: "reference", collection: "helpCategories" } }),
      order: z.number().default(0),
    });
  },
};

/** A help category, the groups on the Help home. Served at `/help/category/{id}/`. */
const helpCategories = {
  type: "content_layer",
  loader: orsoGlob({ base: "./src/content/helpCategories/en", pattern: "*.md" }),
  schema: () => {
    const z = getZod();
    return z.object({
      title: z.string(),
      description: z.string().optional(),
      order: z.number().default(0),
    });
  },
};

/** `orso({ collections })` entries for the help centre. */
export const help = {
  helpDocs: { collection: helpDocs, previewPath: ({ slug }) => `/help/${slug}/` },
  helpCategories: { collection: helpCategories, previewPath: ({ slug }) => `/help/category/${slug}/` },
};
