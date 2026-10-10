import { defineStore } from "pinia";
import { computed, onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { ContentCollection, ContentEntry, ContentEntrySummary } from "@/core/types";
import { errorMessage } from "@/lib/errors";

export const useContentStore = defineStore("content", () => {
  const daemon = useDaemon();
  const projectId = ref("");
  const collections = ref<ContentCollection[]>([]);
  const entries = ref<ContentEntrySummary[]>([]);
  const selectedCollection = ref<string | null>(null);
  const entry = ref<ContentEntry | null>(null);
  const draftSource = ref("");
  const dirty = computed(() => !!entry.value && draftSource.value !== entry.value.source);
  const changedOnDisk = ref(false);
  const error = ref<string | null>(null);
  const loading = ref(false);
  let selectionRequest = 0;
  let refreshRequest = 0;
  let selectionPending = false;
  function canLeave() {
    if (!dirty.value) return true;
    if (!window.confirm("Discard unsaved content changes?")) return false;
    draftSource.value = entry.value?.source ?? "";
    return true;
  }
  async function open(id: string) {
    if (id === projectId.value) return;
    if (!canLeave()) return;
    ++selectionRequest;
    ++refreshRequest;
    selectionPending = false;
    projectId.value = id; selectedCollection.value = null; entry.value = null; entries.value = []; collections.value = [];
    draftSource.value = ""; changedOnDisk.value = false;
    await refresh();
  }
  async function refresh() {
    const id = projectId.value;
    if (!id) return;
    const version = ++refreshRequest;
    const selectionVersion = selectionRequest;
    const name = selectedCollection.value;
    const openedEntry = entry.value;
    const canRefreshEntry = !selectionPending;
    loading.value = true; error.value = null;
    try {
      const nextCollections = await daemon.api.listContentCollections(id);
      const nextEntries = name ? await daemon.api.listContentEntries(id, name) : [];
      if (version !== refreshRequest) return;
      collections.value = nextCollections;
      // A refresh owns catalog loading; a user selection owns the detail view.
      if (selectionVersion !== selectionRequest) return;
      entries.value = nextEntries;
      if (openedEntry && canRefreshEntry && !selectionPending) {
        if (dirty.value) { changedOnDisk.value = true; return; }
        const next = await daemon.api.contentEntry(id, openedEntry.collection, openedEntry.id);
        if (version !== refreshRequest || selectionVersion !== selectionRequest || dirty.value) return;
        entry.value = next; draftSource.value = next.source;
      }
    } catch (e) { if (version === refreshRequest && selectionVersion === selectionRequest) error.value = errorMessage(e); }
    finally { if (version === refreshRequest) loading.value = false; }
  }
  async function selectCollection(name: string) {
    if (!canLeave()) return;
    const version = ++selectionRequest; selectionPending = true;
    selectedCollection.value = name; entry.value = null; draftSource.value = ""; entries.value = []; changedOnDisk.value = false; error.value = null;
    try { const next = await daemon.api.listContentEntries(projectId.value, name); if (version === selectionRequest) entries.value = next; }
    catch(e) { if (version === selectionRequest) error.value = errorMessage(e); }
    finally { if (version === selectionRequest) selectionPending = false; }
  }
  async function selectEntry(id: string) {
    if (!selectedCollection.value || !canLeave()) return;
    const version = ++selectionRequest; selectionPending = true; error.value = null;
    try {
      const next = await daemon.api.contentEntry(projectId.value, selectedCollection.value, id);
      if (version !== selectionRequest) return;
      entry.value = next; draftSource.value = next.source; changedOnDisk.value = false;
    } catch(e) { if (version === selectionRequest) error.value = errorMessage(e); }
    finally { if (version === selectionRequest) selectionPending = false; }
  }
  const offs = [
    daemon.on("fs.change", (e) => { if(e.projectId === projectId.value) void refresh(); }),
    daemon.on("events.lagged", () => void refresh()),
    daemon.onReconnected(() => void refresh()),
  ];
  onScopeDispose(() => offs.forEach(off => off()));
  return { projectId, collections, entries, selectedCollection, entry, draftSource, dirty, changedOnDisk, error, loading, open, refresh, selectCollection, selectEntry, canLeave };
});
