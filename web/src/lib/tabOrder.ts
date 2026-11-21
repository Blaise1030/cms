import { ref } from "vue";
import type { ProjectTab } from "@/router";

/**
 * The viewer's order for the project tabs (the rail's drag-to-reorder). Opening a project
 * lands on the first tab in this order. The daemon keeps it (the desktop UI's origin, and so
 * its localStorage, changes on every launch); localStorage is the copy read synchronously.
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

function readSaved(): unknown {
  try {
    return JSON.parse(localStorage.getItem(TAB_ORDER_KEY) ?? "null");
  } catch {
    // No storage (private window) or a bad value: nothing saved.
    return null;
  }
}

function writeSaved(ids: readonly ProjectTab[]) {
  try {
    localStorage.setItem(TAB_ORDER_KEY, JSON.stringify(ids));
  } catch {
    // Not persisted here; the order still holds for this page.
  }
}

/** Every tab exactly once: saved ids first (unknown ones dropped), then any tab added since. */
export function normalizeTabOrder(saved: unknown): ProjectTab[] {
  const known = new Set<ProjectTab>(DEFAULT_TAB_ORDER);
  const ordered = Array.isArray(saved)
    ? saved.filter((id): id is ProjectTab => known.delete(id as ProjectTab))
    : [];
  return [...ordered, ...DEFAULT_TAB_ORDER.filter((id) => known.has(id))];
}

export function loadTabOrder(): ProjectTab[] {
  return normalizeTabOrder(readSaved());
}

/** Where the order is kept beyond this browser (the daemon). */
export interface TabOrderStore {
  load(): Promise<string[] | null>;
  save(order: string[]): Promise<unknown>;
}

let remote: TabOrderStore | null = null;
/** Bumped when the daemon's order replaces the local copy, so a mounted rail re-reads it. */
export const tabOrderChanged = ref(0);

/**
 * Read the order the daemon keeps into the local copy. With none saved there yet, a local
 * order (from before the daemon kept it) is sent up, so nobody loses theirs. Never throws.
 */
export async function connectTabOrder(store: TabOrderStore): Promise<void> {
  remote = store;
  let saved: string[] | null;
  try {
    saved = await store.load();
  } catch {
    return;
  }
  if (Array.isArray(saved)) {
    writeSaved(normalizeTabOrder(saved));
    tabOrderChanged.value++;
  } else if (Array.isArray(readSaved())) {
    void store.save(loadTabOrder()).catch(() => undefined);
  }
}

export function saveTabOrder(ids: ProjectTab[]) {
  writeSaved(ids);
  void remote?.save(ids).catch(() => undefined);
}

/** The tab a project opens on. */
export function firstTab(): ProjectTab {
  return loadTabOrder()[0]!;
}
