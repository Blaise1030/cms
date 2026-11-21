import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { translatedLocales } from "../i18n/locales";

/**
 * Whether Crowdin has written any translated Markdown for `collection` in `locale`
 * (`src/content/<collection>/<locale>/*.md`). Paths resolve from the site root, where Astro runs.
 * Build-time only: never import this from a Vue island or a client script.
 */
export function hasTranslatedFiles(collection: string, locale: string) {
  const dir = join(process.cwd(), "src", "content", collection, locale);
  return existsSync(dir) && readdirSync(dir).some((file) => file.endsWith(".md"));
}

/** The English collections Crowdin translates. */
export const translatedCollections = ["blog", "blogCategories", "helpDocs", "helpCategories", "helpSubcategories", "helpDocsCategories", "legal"];

/** The collections with translated files in some locale: each gets a `<name>I18n` collection. */
export const collectionsWithTranslations = () =>
  translatedCollections.filter((name) => translatedLocales.some((locale) => hasTranslatedFiles(name, locale)));
