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

/** Help articles, served at `/help/{id}/`. `category` names the help category they are listed under. */
const helpDocs = contentCollection({
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
});

/**
 * Help categories, the groups on the Help home. Served at `/help/category/{id}/`. An article
 * whose `category` names no category file still gets a category of that name.
 */
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

/** Marketing-site collection preset for `orso({ collections: marketingCollections })`. */
export const marketingCollections = attachMarketingPreviewPaths({
  blog,
  blogCategories,
  helpDocs,
  helpCategories,
});

export { attachMarketingPreviewPaths, marketingPreviewPaths } from "./marketing-preview-paths.js";