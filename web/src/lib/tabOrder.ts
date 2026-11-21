import type { ProjectTab } from "@/router";

/**
 * The viewer's order for the project tabs (the rail's drag-to-reorder), kept in this browser
 * only. Opening a project lands on the first tab in this order.
 */
export const TAB_ORDER_KEY = "orso.rail.order";

export const DEFAULT_TAB_ORDER: readonly ProjectTab[] = [
  "preview",
  "content",
  "design",
  "code",
  "history",
  "review",
  "settings",
];

/** Every tab exactly once: saved ids first (unknown ones dropped), then any tab added since. */
export function loadTabOrder(): ProjectTab[] {
  let saved: unknown = null;
  try {
    saved = JSON.parse(localStorage.getItem(TAB_ORDER_KEY) ?? "null");
  } catch {
    // No storage (private window) or a bad value: the default order.
  }
  const known = new Set<ProjectTab>(DEFAULT_TAB_ORDER);
  const ordered = Array.isArray(saved)
    ? saved.filter((id): id is ProjectTab => known.delete(id as ProjectTab))
    : [];
  return [...ordered, ...DEFAULT_TAB_ORDER.filter((id) => known.has(id))];
}

export function saveTabOrder(ids: ProjectTab[]) {
  try {
    localStorage.setItem(TAB_ORDER_KEY, JSON.stringify(ids));
  } catch {
    // Not persisted; the order still holds for this page.
  }
}

/** The tab a project opens on. */
export function firstTab(): ProjectTab {
  return loadTabOrder()[0]!;
}
