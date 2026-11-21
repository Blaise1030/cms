import { createRequire } from "node:module";

import { orsoGlob } from "./orso/index.js";

const require = createRequire(import.meta.url);
const getZod = () => require("astro/zod").z;

/**
 * The privacy policy and terms (`src/content/legal/en/*.md`), editable in Orso. Each entry is
 * served at `/<id>/` by a page of the same name in `src/pages/[...locale]/`, and Crowdin
 * translates it like the blog. Kept out of `integrations/orso/`, which Orso may overwrite.
 */
const collection = {
  type: "content_layer",
  loader: orsoGlob({ base: "./src/content/legal/en", pattern: "*.md" }),
  schema: () => {
    const z = getZod();
    return z.object({
      title: z.string(),
      description: z.string(),
      updated: z
        .string()
        .regex(/^\d{4}-\d{2}-\d{2}$/, "Use YYYY-MM-DD")
        .refine(
          (value) => {
            const date = new Date(`${value}T00:00:00Z`);
            return Number.isFinite(date.getTime()) && date.toISOString().slice(0, 10) === value;
          },
          "Use a valid calendar date",
        )
        .meta({ editor: { type: "date", knownRefinement: true } }),
    });
  },
};

/** `orso({ collections })` entry for the legal pages. */
export const legal = { collection, previewPath: ({ slug }) => `/${slug}/` };
