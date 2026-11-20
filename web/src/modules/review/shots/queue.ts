import { computed, ref, type Ref } from "vue";
import type { CssRect } from "./crop";

/** A screenshot comment waiting to be sent with the next review. */
export interface ShotItem {
  id: string;
  side: "change" | "live";
  /** The page it was taken on, e.g. `/pricing`. */
  path: string;
  /** "desktop", "tablet 768px"…, as the frame was showing it. */
  viewport: string;
  rect: CssRect;
  /** `data:image/png;base64,…`; null for a note added without a screenshot. */
  imageDataUrl: string | null;
  note: string;
  createdAt: string;
}

const DB = "orso-review-shots";
const STORE = "queues";

/** Queues already loaded this session, shared by every view of the same change. */
const queues = new Map<string, Ref<ShotItem[]>>();
let db: Promise<IDBDatabase | null> | null = null;

function openDb(): Promise<IDBDatabase | null> {
  db ??= new Promise((resolve) => {
    try {
      if (typeof indexedDB === "undefined") return resolve(null);
      const req = indexedDB.open(DB, 1);
      req.onupgradeneeded = () => req.result.createObjectStore(STORE);
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => resolve(null);
      req.onblocked = () => resolve(null);
    } catch {
      resolve(null);
    }
  });
  return db;
}

async function read(key: string): Promise<ShotItem[] | null> {
  const d = await openDb();
  if (!d) return null;
  return new Promise((resolve) => {
    try {
      const req = d.transaction(STORE, "readonly").objectStore(STORE).get(key);
      req.onsuccess = () => resolve(Array.isArray(req.result) ? (req.result as ShotItem[]) : null);
      req.onerror = () => resolve(null);
    } catch {
      resolve(null);
    }
  });
}

async function write(key: string, items: ShotItem[]): Promise<void> {
  const d = await openDb();
  if (!d) return;
  try {
    const store = d.transaction(STORE, "readwrite").objectStore(STORE);
    // Plain copies: reactive proxies can't be structured-cloned.
    if (items.length) store.put(JSON.parse(JSON.stringify(items)), key);
    else store.delete(key);
  } catch {
    // Kept in memory for this session.
  }
}

/** Test hook: forget queues held in memory (IndexedDB is left alone). */
export function resetShotQueuesForTest() {
  queues.clear();
  db = null;
}

let seq = 0;
export const newShotId = () => `shot-${Date.now().toString(36)}-${(++seq).toString(36)}`;

/**
 * The screenshot comments queued on change `number` of `projectId`. Saved in IndexedDB so a
 * reload keeps them (in memory when storage isn't available).
 */
export function useShotQueue(projectId: () => string, number: () => number) {
  const key = computed(() => `${projectId()}#${number()}`);
  const loading = ref(false);

  /** The queue every view of this change shares, read from IndexedDB the first time. */
  function held(k: string): Ref<ShotItem[]> {
    let q = queues.get(k);
    if (q) return q;
    const created = ref<ShotItem[]>([]);
    queues.set(k, created);
    loading.value = true;
    void read(k).then((stored) => {
      // Something may have been added while reading.
      if (stored?.length) created.value = [...stored, ...created.value.filter((i) => !stored.some((s) => s.id === i.id))];
      loading.value = false;
    });
    return created;
  }
  const shared = computed(() => held(key.value));
  const items = computed(() => shared.value.value);

  function set(next: ShotItem[]) {
    shared.value.value = next;
    void write(key.value, next);
  }

  return {
    items,
    loading,
    add: (item: ShotItem) => set([...items.value, item]),
    update: (id: string, change: Partial<Pick<ShotItem, "note">>) =>
      set(items.value.map((i) => (i.id === id ? { ...i, ...change } : i))),
    remove: (id: string) => set(items.value.filter((i) => i.id !== id)),
    clear: () => set([]),
  };
}
