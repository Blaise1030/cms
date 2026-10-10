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
  let request = 0;
  function canLeave() {
    if (!dirty.value) return true;
    if (!window.confirm("Discard unsaved content changes?")) return false;
    draftSource.value = entry.value?.source ?? "";
    return true;
  }
  async function open(id: string) {
    if (id === projectId.value) return;
    if (!canLeave()) return;
    ++request;
    projectId.value = id; selectedCollection.value = null; entry.value = null; entries.value = [];
    draftSource.value = ""; changedOnDisk.value = false;
    await refresh();
  }
  async function refresh() {
    const id = projectId.value;
    if (!id) return;
    const version = ++request;
    loading.value = true; error.value = null;
    try {
      const nextCollections = await daemon.api.listContentCollections(id);
      const name = selectedCollection.value;
      const nextEntries = name ? await daemon.api.listContentEntries(id, name) : [];
      if (version !== request) return;
      collections.value = nextCollections; entries.value = nextEntries;
      if (entry.value) {
        if (dirty.value) { changedOnDisk.value = true; return; }
        const next = await daemon.api.contentEntry(id, entry.value.collection, entry.value.id);
        if (version !== request || dirty.value) return;
        entry.value = next; draftSource.value = next.source;
      }
    } catch (e) { if (version === request) error.value = errorMessage(e); }
    finally { if (version === request) loading.value = false; }
  }
  async function selectCollection(name: string) {
    if (!canLeave()) return;
    const version = ++request;
    selectedCollection.value = name; entry.value = null; draftSource.value = ""; entries.value = []; changedOnDisk.value = false; error.value = null;
    try { const next = await daemon.api.listContentEntries(projectId.value, name); if (version === request) entries.value = next; }
    catch(e) { if (version === request) error.value = errorMessage(e); }
  }
  async function selectEntry(id: string) {
    if (!selectedCollection.value || !canLeave()) return;
    const version = ++request; error.value = null;
    try {
      const next = await daemon.api.contentEntry(projectId.value, selectedCollection.value, id);
      if (version !== request) return;
      entry.value = next; draftSource.value = next.source; changedOnDisk.value = false;
    } catch(e) { if (version === request) error.value = errorMessage(e); }
  }
  const offs = [
    daemon.on("fs.change", (e) => { if(e.projectId === projectId.value) void refresh(); }),
    daemon.on("events.lagged", () => void refresh()),
    daemon.onReconnected(() => void refresh()),
  ];
  onScopeDispose(() => offs.forEach(off => off()));
  return { projectId, collections, entries, selectedCollection, entry, draftSource, dirty, changedOnDisk, error, loading, open, refresh, selectCollection, selectEntry, canLeave };
});
