import type { Component } from "vue";
import type { ContentCollection, ContentEntrySummary } from "@/core/types";

/** What a tree row stands for: a collection folder, or an entry in one. */
export interface TreeTarget { collection: string; id?: string }
/** One row of a tree menu; `"separator"` draws a divider. */
export type TreeAction = "separator" | {
  id: string;
  label: string;
  icon?: Component;
  destructive?: boolean;
  disabled?: boolean;
  /** Why the action is disabled, shown under it. */
  hint?: string | null;
  run: () => void;
};

/** Tree path of an entry: `blog/welcome`; nested ids nest (`docs/guides/setup`). */
export const entryTreePath = (collection: string, id: string) => `${collection}/${id}`;

/** Every collection as a folder (so empty ones show) and every entry inside it. */
export function treePaths(collections: readonly ContentCollection[], entries: Readonly<Record<string, readonly ContentEntrySummary[]>>): string[] {
  return collections.flatMap(c => [`${c.name}/`, ...(entries[c.name] ?? []).map(e => entryTreePath(c.name, e.id))]);
}

/** The collection or entry a tree row stands for; folders inside a collection stand for neither. */
export function treeTarget(path: string, kind: "file" | "directory"): TreeTarget | null {
  const clean = path.replace(/\/$/, "");
  const slash = clean.indexOf("/");
  if (kind === "directory") return slash < 0 ? { collection: clean } : null;
  return slash < 0 ? null : { collection: clean.slice(0, slash), id: clean.slice(slash + 1) };
}
