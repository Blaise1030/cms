---
title: Make your content collections editable
description: Load your collections with orsoGlob so they appear in the Content tab.
category: existing-site
order: 4
---

orso can edit a content collection when it is loaded with `orsoGlob` and has a schema. Collections loaded another way still show in the Content tab, but as read-only. For editing entries, see [Edit blog posts and other collections](/help/edit-blog-posts-and-collections/).

## Switch the loader

```ts
import { defineCollection } from "astro:content";
import { z } from "astro/zod";
import { orsoGlob } from "../integrations/orso/index.js";

const blog = defineCollection({
  loader: orsoGlob({ base: "./src/content/blog", pattern: "*.md" }),
  schema: z.object({
    title: z.string(),
    pubDate: z.coerce.date(),
  }),
});
```

## Rules

- The pattern must be exactly `*.md`, `*.mdx` or `*.json`.
- Files in nested folders (`**/*.md`) are read-only. Give each folder its own collection instead.
- The collection needs a schema, and `base` must be inside the repo.
- For a single settings file, use `orsoGlob({ base, pattern: "*.json", singleton: "site" })`.

## How fields appear

orso builds the editing form from your schema. Each field gets an input that suits its type, such as text, dates, numbers, lists or images. The body of a Markdown file opens in the rich-text editor.

## Open the right page in the preview

To make **Open preview** jump to an entry's page, register the collection with a preview path in `astro.config.mjs`:

```js
orso({
  collections: {
    blog: { collection: blog, previewPath: ({ slug }) => `/blog/${slug}/` },
  },
});
```

Then re-export the collections from `src/content.config.ts`:

```ts
export { collections } from "virtual:orso/collections";
```
