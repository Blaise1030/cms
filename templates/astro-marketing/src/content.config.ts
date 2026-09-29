import { defineCollection } from "astro:content";
import { glob } from "astro/loaders";
import { z } from "astro/zod";

const link = z.object({ label: z.string(), href: z.string() });

/** `src/content/site.json` — global site settings (single entry, id "site"). */
const site = defineCollection({
  loader: glob({ base: "./src/content", pattern: "site.json" }),
  schema: z.object({
    name: z.string(),
    description: z.string(),
    nav: z.array(link),
    footer: z.object({ text: z.string(), links: z.array(link).default([]) }),
    socials: z.array(link).default([]),
  }),
});

/** `src/content/pages/*.json` — a page is an ordered list of section ids. */
const pages = defineCollection({
  loader: glob({ base: "./src/content/pages", pattern: "*.json" }),
  schema: z.object({
    title: z.string(),
    description: z.string(),
    /** Ids (file names without extension) of entries in `sections`. */
    sections: z.array(z.string()),
  }),
});

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
export const sectionSchema = z.discriminatedUnion("type", [
  hero,
  features,
  pricing,
  testimonials,
  faq,
  cta,
  logos,
]);

const sections = defineCollection({
  loader: glob({ base: "./src/content/sections", pattern: "*.mdx" }),
  schema: sectionSchema,
});

export const collections = { site, pages, sections };
