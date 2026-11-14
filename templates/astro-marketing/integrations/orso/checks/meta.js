export const TITLE_MAX = 60;
export const DESCRIPTION_MAX = 160;

/**
 * @typedef {object} PageMeta
 * @property {string} title
 * @property {number} titleLength
 * @property {string} description
 * @property {number} descriptionLength
 * @property {string} canonical
 * @property {string} robots
 * @property {string} lang
 * @property {string} h1
 * @property {number} h1Count
 * @property {string} ogTitle
 * @property {string} ogDescription
 * @property {string} ogImage
 * @property {string[]} jsonLdTypes
 * @property {string[]} jsonLdBlocks
 */

/**
 * Read the SEO fields a reviewer usually checks without DevTools.
 *
 * @param {{ querySelector: typeof Document.prototype.querySelector, querySelectorAll: typeof Document.prototype.querySelectorAll }} doc
 * @returns {PageMeta}
 */
export function readPageMeta(doc) {
  const title = (doc.querySelector("title")?.textContent ?? "").trim();
  const description = (
    doc.querySelector('meta[name="description"]')?.getAttribute("content") ?? ""
  ).trim();
  const canonical = (
    doc.querySelector('link[rel="canonical"]')?.getAttribute("href") ?? ""
  ).trim();
  const robotsRaw = (
    doc.querySelector('meta[name="robots"]')?.getAttribute("content") ?? ""
  ).trim();
  const lang = (doc.querySelector("html")?.getAttribute("lang") ?? "").trim();
  const h1Els = [...doc.querySelectorAll("h1")];
  const h1 = (h1Els[0]?.textContent ?? "").trim();

  const ogTitle = metaProperty(doc, "og:title");
  const ogDescription = metaProperty(doc, "og:description");
  const ogImage = metaProperty(doc, "og:image");

  const jsonLdTypes = [];
  const jsonLdBlocks = [];
  const seen = new Set();
  for (const script of doc.querySelectorAll('script[type="application/ld+json"]')) {
    const raw = (script.textContent ?? "").trim();
    if (!raw) continue;
    try {
      const parsed = JSON.parse(raw);
      jsonLdBlocks.push(JSON.stringify(parsed, null, 2));
      for (const type of collectTypes(parsed)) {
        if (seen.has(type)) continue;
        seen.add(type);
        jsonLdTypes.push(type);
      }
    } catch {
      // Invalid JSON-LD is reported by audit rules; snapshot skips it.
    }
  }

  return {
    title,
    titleLength: title.length,
    description,
    descriptionLength: description.length,
    canonical,
    robots: robotsRaw || "index, follow",
    lang,
    h1,
    h1Count: h1Els.length,
    ogTitle,
    ogDescription,
    ogImage,
    jsonLdTypes,
    jsonLdBlocks,
  };
}

/**
 * @param {{ querySelector: typeof Document.prototype.querySelector }} doc
 * @param {string} property
 */
function metaProperty(doc, property) {
  return (
    doc.querySelector(`meta[property="${property}"]`)?.getAttribute("content") ?? ""
  ).trim();
}

/**
 * @param {unknown} node
 * @returns {string[]}
 */
function collectTypes(node) {
  /** @type {string[]} */
  const types = [];
  walkJsonLd(node, types);
  return types;
}

/**
 * @param {unknown} node
 * @param {string[]} types
 */
function walkJsonLd(node, types) {
  if (node == null || typeof node !== "object") return;

  if (Array.isArray(node)) {
    for (const item of node) walkJsonLd(item, types);
    return;
  }

  /** @type {Record<string, unknown>} */
  const obj = /** @type {Record<string, unknown>} */ (node);

  const atType = obj["@type"];
  if (typeof atType === "string") {
    types.push(atType);
  } else if (Array.isArray(atType)) {
    for (const t of atType) {
      if (typeof t === "string") types.push(t);
    }
  }

  if (Array.isArray(obj["@graph"])) {
    walkJsonLd(obj["@graph"], types);
  }
}
