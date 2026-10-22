/**
 * Test fixture mirroring the schemas in `templates/astro-marketing/src/content.config.ts`.
 *
 * It is intentionally a copy rather than an import: `content.config.ts` imports
 * `astro:content`, and the package must describe the same schemas the template uses.
 */
export const collectionRegistry = {
  blog: { loader: { base: "./src/content/blog/en", pattern: "*.md" }, kind: "markdown" },
  helpDocs: { loader: { base: "./src/content/helpDocs/en", pattern: "*.md" }, kind: "markdown" },
};

/** Inject Astro's Zod and image context, or Zod alone for Node validation. */
export function createCollectionSchemas({ z, image = () => z.string() }) {
  const blog = z.object({
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
  }).meta({ editor: { knownRefinement: true } });
  const helpDocs = z.object({
    title: z.string(), description: z.string(), section: z.string(), order: z.number(),
  });
  return { blog, helpDocs };
}
