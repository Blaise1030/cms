/**
 * A field or key name as sentence-case words: `coverAlt` → "Cover alt", `seo_title` → "Seo title",
 * `imageURL` → "Image URL". Acronyms keep their capitals.
 */
export function humanize(name: string): string {
  const words = name
    .replace(/([a-z\d])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .split(/[\s_.-]+/)
    .filter(Boolean)
    .map(word => (word.length > 1 && word === word.toUpperCase() ? word : word.toLowerCase()));
  const sentence = words.join(" ");
  return sentence.charAt(0).toUpperCase() + sentence.slice(1);
}

/** `humanize` for use mid-sentence: "Enter cover alt", but "Enter URL". */
export function humanizeLower(name: string): string {
  const text = humanize(name);
  return /^[A-Z][A-Z\d]/.test(text) ? text : text.charAt(0).toLowerCase() + text.slice(1);
}
