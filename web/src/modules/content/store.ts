import { defineStore } from "pinia";
import { computed, onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import { ApiError } from "@/core/api";
import type { ContentCollection, ContentEntry, ContentEntrySummary, FieldDescriptor } from "@/core/types";
import { errorMessage } from "@/lib/errors";
import { diffDraft, cloneValue } from "./draft";

export interface ContentConflict { reason: "branch_changed" | "stale_content"; message: string }

export const useContentStore = defineStore("content", () => {
  const daemon = useDaemon();
  const projectId = ref("");
  const collections = ref<ContentCollection[]>([]);
  const entries = ref<ContentEntrySummary[]>([]);
  const selectedCollection = ref<string | null>(null);
  const entry = ref<ContentEntry | null>(null);
  const draftSource = ref("");
  /** Editable copy of the loaded entry's fields/body; `entry` itself stays the loaded value. */
  const draftFields = ref<Record<string, unknown>>({});
  const draftBody = ref("");
  const saving = ref(false);
  const saveError = ref<string | null>(null);
  const fieldErrors = ref<Record<string, string>>({});
  const conflict = ref<ContentConflict | null>(null);
  /** The save was refused because the project is on main; waiting for a change to be created. */
  const needsChange = ref(false);
  /** Current on-disk entry fetched for comparison with the draft. */
  const current = ref<ContentEntry | null>(null);
  const descriptor = computed<FieldDescriptor | null>(() => {
    const c = collections.value.find(x => x.name === entry.value?.collection);
    return c?.editable && c.fields?.type === "object" ? c.fields : null;
  });
  const pending = computed(() => entry.value ? diffDraft(entry.value, draftFields.value, draftBody.value, descriptor.value) : null);
  const dirty = computed(() => !!entry.value && (draftSource.value !== entry.value.source || !!pending.value?.changed));
  function clearSaveState() { saveError.value = null; fieldErrors.value = {}; conflict.value = null; needsChange.value = false; current.value = null; }
  function loadEntry(next: ContentEntry) {
    entry.value = next; draftSource.value = next.source;
    draftFields.value = cloneValue(next.fields ?? {}); draftBody.value = next.body ?? "";
    changedOnDisk.value = false; clearSaveState();
  }
  const changedOnDisk = ref(false);
  const error = ref<string | null>(null);
  const loading = ref(false);
  let selectionRequest = 0;
  let refreshRequest = 0;
  let selectionPending = false;
  function canLeave() {
    if (!dirty.value) return true;
    if (!window.confirm("Discard unsaved content changes?")) return false;
    if (entry.value) loadEntry(entry.value);
    return true;
  }
  async function open(id: string) {
    if (id === projectId.value) return;
    if (!canLeave()) return;
    ++selectionRequest;
    ++refreshRequest;
    selectionPending = false;
    projectId.value = id; selectedCollection.value = null; entry.value = null; entries.value = []; collections.value = [];
    draftSource.value = ""; draftFields.value = {}; draftBody.value = ""; changedOnDisk.value = false; clearSaveState();
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
        loadEntry(next);
      }
    } catch (e) { if (version === refreshRequest && selectionVersion === selectionRequest) error.value = errorMessage(e); }
    finally { if (version === refreshRequest) loading.value = false; }
  }
  async function selectCollection(name: string) {
    if (!canLeave()) return;
    const version = ++selectionRequest; selectionPending = true;
    selectedCollection.value = name; entry.value = null; draftSource.value = ""; draftFields.value = {}; draftBody.value = ""; entries.value = []; changedOnDisk.value = false; error.value = null; clearSaveState();
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
      loadEntry(next);
    } catch(e) { if (version === selectionRequest) error.value = errorMessage(e); }
    finally { if (version === selectionRequest) selectionPending = false; }
  }
  /** Send only the changed fields. Throws after recording the failure so the draft stays intact. */
  async function saveDraft(branch?: string): Promise<ContentEntry> {
    const e = entry.value; const change = pending.value;
    if (!e) throw new Error("No entry is open");
    if (!change?.changed) return e;
    const saved = { id: projectId.value, entryId: e.id };
    saving.value = true; clearSaveState();
    try {
      const next = await daemon.api.updateContentEntry({
        project: saved.id, collection: e.collection, id: e.id, branch: branch ?? e.branch, hash: e.hash,
        fields: change.fields, ...(change.body !== undefined ? { body: change.body } : {}), ...(change.unset.length ? { unset: change.unset } : {}),
      });
      if (projectId.value === saved.id && entry.value?.id === saved.entryId) loadEntry(next);
      return next;
    } catch (err) {
      if (err instanceof ApiError) {
        if (err.code === "CHANGE_REQUIRED") needsChange.value = true;
        else if (err.reason === "branch_changed" || err.reason === "stale_content") conflict.value = { reason: err.reason, message: err.message };
        else if (err.details?.length) {
          fieldErrors.value = Object.fromEntries(err.details.map(d => [d.path.join("."), d.message]));
          saveError.value = err.message;
        } else saveError.value = err.message;
      } else saveError.value = errorMessage(err);
      throw err;
    } finally { saving.value = false; }
  }
  /** After Create Change from a main-branch save: retry the same draft on the new branch. */
  async function resumeSave(branch: string) { needsChange.value = false; return saveDraft(branch); }
  async function compare() {
    const e = entry.value; if (!e) return;
    try { current.value = await daemon.api.contentEntry(projectId.value, e.collection, e.id); }
    catch (err) { saveError.value = errorMessage(err); }
  }
  /** Discard the draft in favour of the on-disk entry. */
  async function reload() {
    const e = entry.value; if (!e) return;
    try { loadEntry(await daemon.api.contentEntry(projectId.value, e.collection, e.id)); }
    catch (err) { saveError.value = errorMessage(err); }
  }
  const offs = [
    daemon.on("fs.change", (e) => { if(e.projectId === projectId.value) void refresh(); }),
    daemon.on("events.lagged", () => void refresh()),
    daemon.onReconnected(() => void refresh()),
  ];
  onScopeDispose(() => offs.forEach(off => off()));
  return { projectId, collections, entries, selectedCollection, entry, draftSource, draftFields, draftBody, descriptor, saving, saveError, fieldErrors, conflict, needsChange, current, dirty, changedOnDisk, error, loading, open, refresh, selectCollection, selectEntry, canLeave, saveDraft, resumeSave, compare, reload };
});
