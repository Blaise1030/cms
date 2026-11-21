import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";

/**
 * Whether Crowdin has written any translated Markdown for `collection` in `locale`
 * (`src/content/<collection>/<locale>/*.md`). Paths resolve from the site root, where Astro runs.
 * Build-time only: never import this from a Vue island or a client script.
 */
export function hasTranslatedFiles(collection: string, locale: string) {
  const dir = join(process.cwd(), "src", "content", collection, locale);
  return existsSync(dir) && readdirSync(dir).some((file) => file.endsWith(".md"));
}
