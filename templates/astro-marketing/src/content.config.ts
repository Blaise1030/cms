import { defineCollection } from "astro:content";
import { z } from "astro/zod";
import { orsoGlob } from "orso-astro";

const blog = defineCollection({
  loader: orsoGlob({ base: "./src/content/blog/en", pattern: "*.md" }),
  schema: ({ image }) => z.object({
    title: z.string(),
    description: z.string(),
    pubDate: z.string().regex(/^\d{4}-\d{2}-\d{2}$/, "Use YYYY-MM-DD").refine((value) => {
      const date = new Date(`${value}T00:00:00Z`);
      return Number.isFinite(date.getTime()) && date.toISOString().slice(0, 10) === value;
    }, "Use a valid calendar date").meta({ editor: { type: "date", knownRefinement: true } }),
    coverImage: image().meta({ editor: { type: "image" } }).optional(),
    coverAlt: z.string().optional(),
    tags: z.array(z.string()).default([]),
    relatedPosts: z.array(z.string().meta({ editor: { type: "reference", collection: "blog" } })).default([]),
  }).superRefine((post, ctx) => {
    if (post.coverImage && !post.coverAlt?.trim()) {
      ctx.addIssue({ code: "custom", path: ["coverAlt"], message: "Describe the cover image" });
    }
  }).meta({ editor: { knownRefinement: true } }),
});

const helpDocs = defineCollection({
  loader: orsoGlob({ base: "./src/content/helpDocs/en", pattern: "*.md" }),
  schema: z.object({
    title: z.string(),
    description: z.string(),
    section: z.string(),
    order: z.number(),
  }),
});

export const collections = { blog, helpDocs };
