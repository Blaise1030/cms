/** Loader paths are project-relative; consumed by Astro and the content helper. */
export const collectionRegistry = {
  site: { loader: { base: "./src/content", pattern: "site.json" }, kind: "json", singleton: "site" },
  pages: { loader: { base: "./src/content/pages", pattern: "*.json" }, kind: "json" },
  sections: { loader: { base: "./src/content/sections", pattern: "*.mdx" }, kind: "mdx" },
  blog: { loader: { base: "./src/content/blog/en", pattern: "*.md" }, kind: "markdown" },
  helpDocs: { loader: { base: "./src/content/helpDocs/en", pattern: "*.md" }, kind: "markdown" },
};

/** Inject Astro's Zod and image context, or Zod alone for Node validation. */
export function createCollectionSchemas({ z, image = () => z.string() }) {
  const link = z.object({ label: z.string(), href: z.string() });
  const site = z.object({
    name: z.string(), description: z.string(), nav: z.array(link),
    footer: z.object({ text: z.string(), links: z.array(link).default([]) }),
    socials: z.array(link).default([]),
  });
  const pages = z.object({ title: z.string(), description: z.string(), sections: z.array(z.string().meta({ editor: { type: "reference", collection: "sections" } })) });
  const hero = z.object({
    type: z.literal("hero"),
    headline: z.string(),
    subheadline: z.string().optional(),
    cta: link,
    secondaryCta: link.optional(),
  });

  const features = z.object({
    type: z.literal("features"),
    heading: z.string(),
    items: z.array(z.object({ title: z.string(), description: z.string() })),
  });

  const pricing = z.object({
    type: z.literal("pricing"),
    heading: z.string(),
    plans: z.array(
      z.object({
        name: z.string(),
        price: z.string(),
        period: z.string().optional(),
        features: z.array(z.string()),
        cta: link,
        highlighted: z.boolean().default(false),
      }),
    ),
  });

  const testimonials = z.object({
    type: z.literal("testimonials"),
    heading: z.string(),
    items: z.array(
      z.object({
        quote: z.string(),
        author: z.string(),
        role: z.string().optional(),
      }),
    ),
  });

  const faq = z.object({
    type: z.literal("faq"),
    heading: z.string(),
    items: z.array(z.object({ question: z.string(), answer: z.string() })),
  });

  const cta = z.object({
    type: z.literal("cta"),
    heading: z.string(),
    cta: link,
  });

  const logos = z.object({
    type: z.literal("logos"),
    heading: z.string().optional(),
    items: z.array(z.object({ name: z.string(), url: z.string().optional() })),
  });

  /**
   * `src/content/sections/*.mdx` — one file per section instance. Frontmatter is
   * validated by this discriminated union (on `type`); the MDX body is optional
   * supporting copy rendered by the section component.
   */
  const sectionSchema = z.discriminatedUnion("type", [
    hero,
    features,
    pricing,
    testimonials,
    faq,
    cta,
    logos,
  ]);

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
  return { site, pages, sections: sectionSchema, blog, helpDocs };
}
