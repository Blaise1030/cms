import type { FileTree } from "@pierre/trees";

const PREFIX = "cms:tree-open:";

/** The folders left open in the tree saved under `key`, or null when none were saved yet. */
export function savedExpanded(key: string): string[] | null {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(PREFIX + key) ?? "null");
    return Array.isArray(value) ? value.filter((p): p is string => typeof p === "string") : null;
  } catch {
    return null;
  }
}

/**
 * Saves the tree's open folders under `key` as they change. Folders hidden under a closed
 * parent keep their state, and nothing is saved while a search is expanding matches.
 */
export function trackExpanded(tree: FileTree, key: string): () => void {
  const open = new Set(savedExpanded(key) ?? []);
  let saved = JSON.stringify([...open].sort());
  return tree.subscribe(() => {
    if (tree.getSearchValue()) return;
    for (const row of tree.getVisibleRows(0, tree.getVisibleCount())) {
      if (row.kind !== "directory") continue;
      if (row.isExpanded) open.add(row.path);
      else open.delete(row.path);
    }
    const next = JSON.stringify([...open].sort());
    if (next === saved) return;
    saved = next;
    try { localStorage.setItem(PREFIX + key, next); } catch { /* storage full or blocked: the tree still works */ }
  });
}
