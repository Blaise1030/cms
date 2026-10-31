/** An acronym (all capitals, two letters or more) keeps its case in sentence case. */
const isAcronym = (word: string) => /^[A-Z]{2,}$/.test(word);

/** The first word of a sentence-case name: "getting" → "Getting", "API" → "API". */
const sentenceStart = (word: string) =>
  isAcronym(word) ? word : word.charAt(0).toUpperCase() + word.slice(1).toLowerCase();

/**
 * Writer-facing sentence case for a collection or entry name: `helpDocsCategories` →
 * "Help docs categories", `getting-started` → "Getting started". Identifiers, URLs and file
 * paths keep the raw name; only what a writer reads is humanized.
 */
export function humanize(name: string): string {
  const words = name
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .split(/[\s_-]+/)
    .filter(Boolean);
  if (!words.length) return name;
  return words
    .map((word, index) => index === 0 ? sentenceStart(word) : isAcronym(word) ? word : word.toLowerCase())
    .join(" ");
}
