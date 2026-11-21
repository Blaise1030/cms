import { getCollection, type CollectionEntry } from "astro:content";
import { defaultLocale, type Locale } from "../i18n/locales";
import { hasTranslatedFiles } from "./translatedFiles";

/** The orso collections that Crowdin translates; each has a `<name>I18n` twin in content.config.ts. */
export type TranslatedCollection = "blog" | "blogCategories" | "helpDocs" | "helpCategories" | "helpSubcategories" | "helpDocsCategories" | "legal";

/** Front-matter fields taken from a translation. Everything else stays as in the English file. */
const TEXT_FIELDS = ["title", "description", "coverAlt"] as const;


/**
 * A collection in `locale`: every English entry, with its title, description and body swapped
 * for the translation when Crowdin has written one. Untranslated entries stay in English, so
 * every page exists in every language and ids, URLs and references never change.
 */
export async function getLocalizedCollection<C extends TranslatedCollection>(name: C, locale: Locale): Promise<CollectionEntry<C>[]> {
  const source = (await getCollection(name)) as CollectionEntry<C>[];
  if (locale === defaultLocale) return source;

  // Collections with no translated files have no `<name>I18n` collection (see content.config.ts).
  if (!hasTranslatedFiles(name, locale)) return source;
  const prefix = `${locale}/`;
  const translations = (await getCollection(`${name}I18n` as "blogI18n")).filter((entry) => entry.id.startsWith(prefix));
  const byId = new Map(translations.map((entry) => [entry.id.slice(prefix.length), entry]));

  return source.map((entry) => {
    const translation = byId.get(entry.id);
    if (!translation) return entry;
    const data: Record<string, unknown> = { ...entry.data };
    for (const field of TEXT_FIELDS) {
      const value = translation.data[field];
      if (field in data && typeof value === "string" && value.trim() !== "") data[field] = value;
    }
    // `render()` reads `rendered` (and `filePath` for images in the body), so the translated
    // Markdown renders under the English entry's id.
    return { ...entry, data, body: translation.body, rendered: translation.rendered, filePath: translation.filePath } as CollectionEntry<C>;
  });
}
