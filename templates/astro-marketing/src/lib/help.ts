import { getCollection, type CollectionEntry } from "astro:content";

export type Doc = CollectionEntry<"helpDocs">;

/**
 * A help category (a `helpCategories` entry) and its articles. A doc whose `category` names no
 * category file still gets a category of that name, listed after the others.
 */
export interface Category {
  id: string;
  title: string;
  description?: string;
  /** Every article in the category, in reading order. */
  docs: Doc[];
}

export const slugify = (value: string) =>
  value.toLowerCase().trim().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

type Ordered = { order: number; title: string; id: string };
const byOrder = (a: Ordered, b: Ordered) => a.order - b.order || a.title.localeCompare(b.title) || a.id.localeCompare(b.id);
const docOrder = (a: Doc, b: Doc) =>
  byOrder({ order: a.data.order, title: a.data.title, id: a.id }, { order: b.data.order, title: b.data.title, id: b.id });

/** Categories and the reading order (category, then `order`) of every article. */
export async function loadHelp() {
  const [docs, categoryEntries] = await Promise.all([
    getCollection("helpDocs"),
    getCollection("helpCategories"),
  ]);

  const categories = new Map<string, Ordered & { description?: string }>();
  for (const entry of categoryEntries) {
    categories.set(entry.id, { id: entry.id, title: entry.data.title, description: entry.data.description, order: entry.data.order });
  }
  /** The category a `category` value points at: an entry id, or a free-text name. */
  const resolve = (value: string) => {
    const id = categories.has(value) ? value : slugify(value) || "general";
    if (!categories.has(id)) categories.set(id, { id, title: value, order: Number.MAX_SAFE_INTEGER });
    return id;
  };
  const categoryOfDoc = new Map(docs.map((doc) => [doc.id, resolve(doc.data.category)]));

  const all: Category[] = [...categories.values()].sort(byOrder).map(({ order: _order, ...category }) => ({
    ...category,
    docs: docs.filter((doc) => categoryOfDoc.get(doc.id) === category.id).sort(docOrder),
  }));

  return {
    /** Every category, including ones with no articles yet. */
    categories: all,
    /** Every article in reading order; previous/next navigation follows it. */
    docs: all.flatMap((category) => category.docs),
    categoryOf: (doc: Doc) => all.find((category) => category.id === categoryOfDoc.get(doc.id))!,
  };
}
