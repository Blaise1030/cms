import { defineCollection } from "astro:content";
import { glob } from "astro/loaders";
import { z } from "astro/zod";
import { collections as sourceCollections } from "virtual:orso/collections";
import { translatedLocales } from "./i18n/locales";
import { collectionsWithTranslations } from "./lib/translatedFiles";

/**
 * Translations of the English collections, written by Crowdin into sibling locale folders
 * (`src/content/blog/zh/*.md`). They are plain Astro collections, not orso ones, so the orso
 * editor only ever edits English and Crowdin owns every translation.
 *
 * Only the text fields are read from a translation (see src/lib/localized.ts); dates,
 * categories and images always come from the English file. The schema is loose on purpose, so
 * a translator's slip in a locked field can't break the build. A collection with no translated
 * files yet isn't defined at all, so the build doesn't warn about empty folders. astro.config.mjs
 * hides them from orso (`orso({ hide })`), so the app doesn't warn about them either.
 */
const folders = translatedLocales.length === 1 ? translatedLocales[0] : `{${translatedLocales.join(",")}}`;
const translations = (name: string) =>
  defineCollection({
    loader: glob({
      base: `./src/content/${name}`,
      pattern: `${folders}/*.md`,
      // "zh/plan-a-launch-in-a-week": locale, then the English entry id.
      generateId: ({ entry }) => entry.replace(/\.md$/, ""),
    }),
    schema: z
      .object({
        title: z.string().optional(),
        description: z.string().optional(),
        coverAlt: z.string().optional(),
      })
      .passthrough(),
  });

export const collections = {
  ...sourceCollections,
  ...Object.fromEntries(collectionsWithTranslations().map((name) => [`${name}I18n`, translations(name)])),
};
