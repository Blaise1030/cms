import { previewSitePath } from "./previewAsset";

export const TITLE_MAX = 60;
export const DESCRIPTION_MAX = 160;

export type PageMeta = {
  title: string;
  titleLength: number;
  description: string;
  descriptionLength: number;
  canonical: string;
  robots: string;
  lang: string;
  h1: string;
  h1Count: number;
  favicon: string;
  ogTitle: string;
  ogDescription: string;
  ogImage: string;
  jsonLdTypes: string[];
  /** Pretty-printed JSON-LD documents found on the page (invalid blocks omitted). */
  jsonLdBlocks: string[];
};

/** Read the SEO fields a reviewer usually checks without DevTools. */
export function readPageMeta(doc: ParentNode, opts: { baseUrl?: string } = {}): PageMeta {
  const base =
    opts.baseUrl ||
    (typeof Document !== "undefined" && doc instanceof Document ? doc.baseURI : "") ||
    undefined;

  const title = (doc.querySelector("title")?.textContent ?? "").trim();
  const description = (
    doc.querySelector('meta[name="description"]')?.getAttribute("content") ?? ""
  ).trim();
  // Paths only — strip Orso's /preview/{id}/ mount so labels match the public site.
  const canonical = previewSitePath(
    (doc.querySelector('link[rel="canonical"]')?.getAttribute("href") ?? "").trim(),
    base,
  );
  const robotsRaw = (
    doc.querySelector('meta[name="robots"]')?.getAttribute("content") ?? ""
  ).trim();
  const lang = (doc.querySelector("html")?.getAttribute("lang") ?? "").trim();
  const h1Els = [...doc.querySelectorAll("h1")];
  const h1 = (h1Els[0]?.textContent ?? "").trim();

  const ogTitle = metaProperty(doc, "og:title");
  const ogDescription = metaProperty(doc, "og:description");
  const ogImage = previewSitePath(metaProperty(doc, "og:image"), base);
  const favicon = previewSitePath(readFaviconHref(doc), base);

  const jsonLdTypes: string[] = [];
  const jsonLdBlocks: string[] = [];
  const seen = new Set<string>();
  for (const script of doc.querySelectorAll('script[type="application/ld+json"]')) {
    const raw = (script.textContent ?? "").trim();
    if (!raw) continue;
    try {
      const parsed = JSON.parse(raw) as unknown;
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
    favicon,
    ogTitle,
    ogDescription,
    ogImage,
    jsonLdTypes,
    jsonLdBlocks,
  };
}

function readFaviconHref(doc: ParentNode): string {
  const rels = ["icon", "shortcut icon", "apple-touch-icon", "apple-touch-icon-precomposed"];
  for (const rel of rels) {
    const href = doc.querySelector(`link[rel="${rel}"]`)?.getAttribute("href")?.trim();
    if (href) return href;
  }
  // `rel` can list multiple tokens: rel="icon shortcut".
  for (const link of doc.querySelectorAll("link[rel]")) {
    const rel = (link.getAttribute("rel") ?? "").toLowerCase().split(/\s+/);
    if (rel.includes("icon") || rel.includes("apple-touch-icon")) {
      const href = link.getAttribute("href")?.trim();
      if (href) return href;
    }
  }
  return "";
}

function metaProperty(doc: ParentNode, property: string): string {
  return (
    doc.querySelector(`meta[property="${property}"]`)?.getAttribute("content") ?? ""
  ).trim();
}

function collectTypes(node: unknown): string[] {
  const types: string[] = [];
  walkJsonLd(node, types);
  return types;
}

function walkJsonLd(node: unknown, types: string[]): void {
  if (node == null || typeof node !== "object") return;

  if (Array.isArray(node)) {
    for (const item of node) walkJsonLd(item, types);
    return;
  }

  const obj = node as Record<string, unknown>;
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
