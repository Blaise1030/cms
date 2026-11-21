import { createRequire } from "node:module";

import { orsoGlob } from "./index.js";
import { attachMarketingPreviewPaths } from "./marketing-preview-paths.js";

const require = createRequire(import.meta.url);
const getZod = () => require("astro/zod").z;

/** @param {{ loader: unknown, schema: unknown }} config */
function contentCollection(config) {
  return { ...config, type: "content_layer" };
}

const blog = contentCollection({
  loader: orsoGlob({ base: "./src/content/blog/en", pattern: "*.md" }),
  schema: ({ image }) => {
    const z = getZod();
    return z
      .object({
        title: z.string(),
        description: z.string(),
        pubDate: z
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
        coverImage: image().meta({ editor: { type: "image" } }).optional(),
        coverAlt: z.string().optional(),
        categories: z
          .array(z.string().meta({ editor: { type: "reference", collection: "blogCategories" } }))
          .default([]),
        tags: z.array(z.string()).default([]),
        relatedPosts: z
          .array(z.string().meta({ editor: { type: "reference", collection: "blog" } }))
          .default([]),
      })
      .superRefine((post, ctx) => {
        if (post.coverImage && !post.coverAlt?.trim()) {
          ctx.addIssue({ code: "custom", path: ["coverAlt"], message: "Describe the cover image" });
        }
      })
      .meta({ editor: { knownRefinement: true } });
  },
});

/** Blog categories: `blog.categories` lists their entry ids. Routed at `/blog/categories/{id}/`. */
const blogCategories = contentCollection({
  loader: orsoGlob({ base: "./src/content/blogCategories/en", pattern: "*.md" }),
  schema: () => {
    const z = getZod();
    return z.object({
      title: z.string(),
      description: z.string().optional(),
    });
  },
});

/**
 * Help articles. `section` names the help section the article belongs to (a `helpCategories`
 * entry id; free text such as "Getting started" still validates, for sites made before sections
 * were a collection). `category` and `subcategory` are optional.
 */
const helpDocs = contentCollection({
  loader: orsoGlob({ base: "./src/content/helpDocs/en", pattern: "*.md" }),
  schema: () => {
    const z = getZod();
    return z.object({
      title: z.string(),
      description: z.string(),
      section: z.string().meta({ editor: { type: "reference", collection: "helpCategories" } }),
      category: z
        .string()
        .meta({ editor: { type: "reference", collection: "helpDocsCategories" } })
        .optional(),
      subcategory: z
        .string()
        .meta({ editor: { type: "reference", collection: "helpSubcategories" } })
        .optional(),
      order: z.number(),
    });
  },
});

/** Help sections, the top-level groups on the Help home. Routed at `/help/section/{id}/`. */
const helpCategories = contentCollection({
  loader: orsoGlob({ base: "./src/content/helpCategories/en", pattern: "*.md" }),
  schema: () => {
    const z = getZod();
    return z.object({
      title: z.string(),
      description: z.string().optional(),
      order: z.number().default(0),
    });
  },
});

/** Groups of articles inside one help section; listed on that section's page. */
const helpSubcategories = contentCollection({
  loader: orsoGlob({ base: "./src/content/helpSubcategories/en", pattern: "*.md" }),
  schema: () => {
    const z = getZod();
    return z.object({
      title: z.string(),
      category: z.string().meta({ editor: { type: "reference", collection: "helpCategories" } }),
      order: z.number().default(0),
    });
  },
});

/** Cross-cutting help topics (`helpDocs.category`). Routed at `/help/categories/{id}/`. */
const helpDocsCategories = contentCollection({
  loader: orsoGlob({ base: "./src/content/helpDocsCategories/en", pattern: "*.md" }),
  schema: () => {
    const z = getZod();
    return z.object({
      title: z.string(),
      description: z.string().optional(),
      order: z.number().default(0),
    });
  },
});

/** Marketing-site collection preset for `orso({ collections: marketingCollections })`. */
export const marketingCollections = attachMarketingPreviewPaths({
  blog,
  blogCategories,
  helpDocs,
  helpCategories,
  helpSubcategories,
  helpDocsCategories,
});

export { attachMarketingPreviewPaths, marketingPreviewPaths } from "./marketing-preview-paths.js";