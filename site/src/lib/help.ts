import type { CollectionEntry } from "astro:content";
import { defaultLocale, type Locale } from "../i18n/locales";
import { getLocalizedCollection } from "./localized";

export type Doc = CollectionEntry<"helpDocs">;

/** A group of articles inside a section (a `helpSubcategories` entry). */
export interface Subsection {
  id: string;
  title: string;
  docs: Doc[];
}

/**
 * A help section (a `helpCategories` entry). A doc whose `section` names no section file — free
 * text such as "Getting started" — still gets a section of that name, listed after the others.
 */
export interface Section {
  id: string;
  title: string;
  description?: string;
  /** Every article in the section, in reading order. */
  docs: Doc[];
  /** Articles outside any subsection, shown first. */
  loose: Doc[];
  subsections: Subsection[];
}

/** A cross-cutting help topic (a `helpDocsCategories` entry) and its articles. */
export interface Topic {
  id: string;
  title: string;
  description?: string;
  docs: Doc[];
}

export const slugify = (value: string) =>
  value.toLowerCase().trim().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

type Ordered = { order: number; title: string; id: string };
const byOrder = (a: Ordered, b: Ordered) => a.order - b.order || a.title.localeCompare(b.title) || a.id.localeCompare(b.id);
const docOrder = (a: Doc, b: Doc) =>
  byOrder({ order: a.data.order, title: a.data.title, id: a.id }, { order: b.data.order, title: b.data.title, id: b.id });

/**
 * Sections, topics and the reading order (section, then subsection, then `order`) of every
 * article, in `locale` (English where untranslated).
 */
export async function loadHelp(locale: Locale = defaultLocale) {
  const [docs, sectionEntries, subsectionEntries, topicEntries] = await Promise.all([
    getLocalizedCollection("helpDocs", locale),
    getLocalizedCollection("helpCategories", locale),
    getLocalizedCollection("helpSubcategories", locale),
    getLocalizedCollection("helpDocsCategories", locale),
  ]);

  const sections = new Map<string, Ordered & { description?: string }>();
  for (const entry of sectionEntries) {
    sections.set(entry.id, { id: entry.id, title: entry.data.title, description: entry.data.description, order: entry.data.order });
  }
  /** The section a `section` value points at: an entry id, a title-like name of one, or a free-text section. */
  const resolve = (value: string) => {
    const id = sections.has(value) ? value : slugify(value) || "general";
    if (!sections.has(id)) sections.set(id, { id, title: value, order: Number.MAX_SAFE_INTEGER });
    return id;
  };
  const sectionOfDoc = new Map(docs.map((doc) => [doc.id, resolve(doc.data.section)]));
  const subsectionsOf = (sectionId: string) =>
    subsectionEntries
      .filter((entry) => sections.has(entry.data.category) ? entry.data.category === sectionId : slugify(entry.data.category) === sectionId)
      .map((entry) => ({ id: entry.id, title: entry.data.title, order: entry.data.order }))
      .sort(byOrder);

  const all: Section[] = [...sections.values()].sort(byOrder).map((section) => {
    const inSection = docs.filter((doc) => sectionOfDoc.get(doc.id) === section.id).sort(docOrder);
    const subsections = subsectionsOf(section.id)
      .map((sub) => ({ id: sub.id, title: sub.title, docs: inSection.filter((doc) => doc.data.subcategory === sub.id) }))
      .filter((sub) => sub.docs.length > 0);
    const grouped = new Set(subsections.flatMap((sub) => sub.docs));
    const loose = inSection.filter((doc) => !grouped.has(doc));
    return {
      id: section.id,
      title: section.title,
      description: section.description,
      docs: [...loose, ...subsections.flatMap((sub) => sub.docs)],
      loose,
      subsections,
    };
  });

  const topics: Topic[] = topicEntries
    .map((entry) => ({ id: entry.id, title: entry.data.title, description: entry.data.description, order: entry.data.order }))
    .sort(byOrder)
    .map(({ order: _order, ...topic }) => ({ ...topic, docs: docs.filter((doc) => doc.data.category === topic.id).sort(docOrder) }));

  return {
    /** Every section, including ones with no articles yet. */
    sections: all,
    topics,
    /** Every article in reading order; previous/next navigation follows it. */
    docs: all.flatMap((section) => section.docs),
    sectionOf: (doc: Doc) => all.find((section) => section.id === sectionOfDoc.get(doc.id))!,
  };
}
