import type { Component } from "vue";

export interface PaletteItem {
  id: string;
  label: string;
  /** Secondary text (a path, a branch); also searched. */
  hint?: string;
  icon?: Component;
  checked?: boolean;
  /** Keep the palette open after running (the item swaps in another page). */
  stay?: boolean;
  run: () => unknown;
}

export interface PaletteGroup {
  heading: string;
  items: PaletteItem[];
  /** Large lists (files, entries): hidden until the user types, then capped. */
  searchOnly?: boolean;
}

export const SEARCH_LIMIT = 8;

function terms(query: string): string[] {
  return query.toLowerCase().split(/\s+/).filter(Boolean);
}

/** Every term appears in the label or hint, in any order. */
export function matches(item: PaletteItem, query: string): boolean {
  const haystack = `${item.label} ${item.hint ?? ""}`.toLowerCase();
  return terms(query).every((t) => haystack.includes(t));
}

/** Matching items per group, label hits before hint-only hits; empty groups are dropped. */
export function filterGroups(groups: PaletteGroup[], query: string): PaletteGroup[] {
  const q = query.trim();
  const out: PaletteGroup[] = [];
  for (const group of groups) {
    if (group.searchOnly && !q) continue;
    let items = q ? group.items.filter((i) => matches(i, q)) : group.items;
    if (q) {
      const inLabel = (i: PaletteItem) => terms(q).some((t) => i.label.toLowerCase().includes(t));
      items = [...items.filter(inLabel), ...items.filter((i) => !inLabel(i))];
    }
    if (group.searchOnly) items = items.slice(0, SEARCH_LIMIT);
    if (items.length) out.push({ ...group, items });
  }
  return out;
}
