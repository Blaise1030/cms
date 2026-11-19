import type { ReviewItem } from "@/core/types";

/** Review filter: your standing on the change's current version, or your own changes. */
export type ReviewFilter = "new" | "greenlit" | "sent_back" | "yours";
export type InboxSort = "newest" | "oldest" | "updated";

/** A GitHub-style search: `is:`, `author:`, `label:`, `review:` and `sort:` qualifiers plus free text. */
export interface InboxQuery {
  state: "open" | "closed";
  author: string | null;
  label: string | null;
  review: ReviewFilter | null;
  sort: InboxSort;
  /** Everything that isn't a known qualifier, lower-cased. */
  text: string;
}

export const DEFAULT_QUERY = "is:open";
const REVIEWS: ReviewFilter[] = ["new", "greenlit", "sent_back", "yours"];
/** GitHub's sort keys. */
const SORTS: Record<string, InboxSort> = {
  "created-desc": "newest",
  "created-asc": "oldest",
  "updated-desc": "updated",
};
export const SORT_KEY: Record<InboxSort, string> = {
  newest: "created-desc",
  oldest: "created-asc",
  updated: "updated-desc",
};

/** Whitespace-separated tokens; `key:"two words"` and `"two words"` stay one token. */
function tokens(q: string): string[] {
  return q.match(/(?:[^\s"]+:)?"[^"]*"?|\S+/g) ?? [];
}
const unquote = (v: string) => v.replace(/^"|"$/g, "");

export function parseQuery(q: string): InboxQuery {
  const out: InboxQuery = { state: "open", author: null, label: null, review: null, sort: "newest", text: "" };
  const free: string[] = [];
  for (const token of tokens(q)) {
    const m = token.match(/^([a-z]+):(.+)$/i);
    const key = m?.[1]?.toLowerCase();
    const value = m ? unquote(m[2]!) : "";
    if (key === "is" && (value === "open" || value === "closed")) out.state = value;
    else if (key === "author" && value) out.author = value;
    else if (key === "label" && value) out.label = value;
    else if (key === "review" && REVIEWS.includes(value as ReviewFilter)) out.review = value as ReviewFilter;
    else if (key === "sort" && SORTS[value]) out.sort = SORTS[value]!;
    else free.push(unquote(token));
  }
  out.text = free.join(" ").trim().toLowerCase();
  return out;
}

/**
 * `q` with qualifier `key` set to `value` (or removed for null), the way GitHub's menus rewrite
 * the search box: other text stays, the qualifier moves to the end.
 */
export function setQualifier(q: string, key: string, value: string | null): string {
  const kept = tokens(q).filter((t) => !t.toLowerCase().startsWith(`${key}:`));
  if (value !== null) kept.push(`${key}:${/\s/.test(value) ? `"${value}"` : value}`);
  return kept.join(" ");
}

/** The items `query` matches, in its order (state is chosen by which list was loaded). */
export function applyQuery(items: ReviewItem[], query: InboxQuery): ReviewItem[] {
  const eq = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();
  const kept = items.filter((item) => {
    if (query.author && !eq(item.author, query.author)) return false;
    if (query.label && !(item.labels ?? []).some((l) => eq(l.name, query.label!))) return false;
    if (query.review === "yours" && !item.mine) return false;
    if (query.review && query.review !== "yours" && (item.mine || item.myState !== query.review)) return false;
    if (query.text) {
      const text = query.text.replace(/^#/, "");
      const hay = `${item.title} #${item.number} ${item.number} ${item.author}`.toLowerCase();
      if (!text.split(/\s+/).every((word) => hay.includes(word.replace(/^#/, "")))) return false;
    }
    return true;
  });
  const time = (item: ReviewItem, key: "createdAt" | "updatedAt") => Date.parse(item[key] ?? item.updatedAt) || 0;
  const by: Record<InboxSort, (a: ReviewItem, b: ReviewItem) => number> = {
    newest: (a, b) => time(b, "createdAt") - time(a, "createdAt"),
    oldest: (a, b) => time(a, "createdAt") - time(b, "createdAt"),
    updated: (a, b) => time(b, "updatedAt") - time(a, "updatedAt"),
  };
  return [...kept].sort(by[query.sort]);
}
