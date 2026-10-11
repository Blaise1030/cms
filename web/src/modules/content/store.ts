import { defineStore } from "pinia";
import { computed, onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import { ApiError } from "@/core/api";
import type { ContentCollection, ContentEntry, ContentEntrySummary, ContentImpact, FieldDescriptor } from "@/core/types";
import { errorMessage } from "@/lib/errors";
import { diffDraft, cloneValue } from "./draft";

export interface ContentConflict { reason: "branch_changed" | "stale_content"; message: string; branch?: string }
/** A rename or delete awaiting confirmation; `impact` is the daemon's dry run. */
export interface ContentLifecycle {
  action: "rename" | "delete"; collection: string; id: string;
  impact: ContentImpact | null; slug: string; busy: boolean; error: string | null;
}

const RICH_BODY_COLLECTIONS = ["blog", "helpDocs"];

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
  const lifecycle = ref<ContentLifecycle | null>(null);
  /** Which mutation a Create Change resumes. */
  let resumeAction: "save" | "lifecycle" = "save";
  const descriptor = computed<FieldDescriptor | null>(() => {
    const c = collections.value.find(x => x.name === entry.value?.collection);
    return c?.editable && c.fields?.type === "object" ? c.fields : null;
  });
  /** Blog and Help docs Markdown bodies get the rich editor; every other body (section MDX included) stays raw. */
  const richBody = computed(() => !!entry.value && entry.value.kind === "markdown" && RICH_BODY_COLLECTIONS.includes(entry.value.collection));
  /** Body text from the body editor; it only reports real edits, so opening an entry never dirties it. */
  function setBody(body: string) { draftBody.value = body; }
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
    projectId.value = id; selectedCollection.value = null; entry.value = null; lifecycle.value = null; entries.value = []; collections.value = [];
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
    // A rename/delete in flight replaces the entry itself; do not re-read the old id.
    const canRefreshEntry = !selectionPending && !lifecycle.value?.busy;
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
    selectedCollection.value = name; entry.value = null; lifecycle.value = null; draftSource.value = ""; draftFields.value = {}; draftBody.value = ""; entries.value = []; changedOnDisk.value = false; error.value = null; clearSaveState();
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
      lifecycle.value = null; loadEntry(next);
    } catch(e) { if (version === selectionRequest) error.value = errorMessage(e); }
    finally { if (version === selectionRequest) selectionPending = false; }
  }
  /** Send only the changed fields. Throws after recording the failure so the draft stays intact. */
  async function saveDraft(branch?: string): Promise<ContentEntry> {
    const e = entry.value; const change = pending.value;
    if (!e) throw new Error("No entry is open");
    if (!change?.changed) return e;
    const saved = { id: projectId.value, entryId: e.id };
    const submitted = { fields: JSON.stringify(draftFields.value), body: draftBody.value };
    saving.value = true; clearSaveState(); resumeAction = "save";
    try {
      const next = await daemon.api.updateContentEntry({
        project: saved.id, collection: e.collection, id: e.id, branch: branch ?? e.branch, hash: e.hash,
        fields: change.fields, ...(change.body !== undefined ? { body: change.body } : {}), ...(change.unset.length ? { unset: change.unset } : {}),
      });
      if (projectId.value === saved.id && entry.value?.id === saved.entryId) {
        // Edits typed while the request was in flight stay in the draft, rebased on the saved entry.
        if (JSON.stringify(draftFields.value) === submitted.fields && draftBody.value === submitted.body) loadEntry(next);
        else { entry.value = next; draftSource.value = next.source; changedOnDisk.value = false; clearSaveState(); }
      }
      return next;
    } catch (err) {
      recordFailure(err, branch);
      throw err;
    } finally { saving.value = false; }
  }
  /** Map a refused mutation onto Create Change, conflict, or field errors; the draft stays. */
  function recordFailure(err: unknown, branch?: string) {
    if (err instanceof ApiError) {
      if (err.code === "CHANGE_REQUIRED") needsChange.value = true;
      else if (err.reason === "branch_changed" || err.reason === "stale_content") conflict.value = { reason: err.reason, message: err.message, ...(branch ? { branch } : {}) };
      else if (err.details?.some(d => Array.isArray(d.path))) {
        fieldErrors.value = Object.fromEntries(err.details.filter(d => Array.isArray(d.path)).map(d => [d.path.join("."), d.message]));
        saveError.value = err.message;
      } else saveError.value = err.message;
    } else saveError.value = errorMessage(err);
  }
  /** Open the rename/delete confirmation and load its dry-run impact. */
  async function startLifecycle(action: ContentLifecycle["action"]) {
    // A rename rewrites owned paths on disk; a draft would carry the old ones back.
    const e = entry.value; if (!e || dirty.value) return;
    const opened = { action, collection: e.collection, id: e.id, impact: null, slug: "", busy: false, error: null };
    lifecycle.value = opened;
    try {
      const impact = await daemon.api.contentImpact(projectId.value, e.collection, e.id);
      if (lifecycle.value?.action === action && lifecycle.value.id === e.id) lifecycle.value.impact = impact;
    } catch (err) { if (lifecycle.value?.id === e.id) lifecycle.value.error = errorMessage(err); }
  }
  function cancelLifecycle() { lifecycle.value = null; }
  /** Rename or delete the opened entry with its opened branch and hash. Throws after recording the failure. */
  async function confirmLifecycle(branch?: string) {
    const l = lifecycle.value; const e = entry.value;
    if (!l || !e || !l.impact || dirty.value) return;
    const saved = { id: projectId.value, entryId: e.id };
    const target = { project: saved.id, collection: e.collection, id: e.id, branch: branch ?? e.branch, hash: e.hash };
    resumeAction = "lifecycle";
    l.busy = true; l.error = null; clearSaveState();
    const stillOpen = () => projectId.value === saved.id && entry.value?.id === saved.entryId;
    try {
      if (l.action === "rename") {
        const res = await daemon.api.renameContentEntry({ ...target, slug: l.slug.trim() });
        if (stillOpen()) { entries.value = res.entries; loadEntry(res.entry); }
      } else {
        const res = await daemon.api.deleteContentEntry(target);
        if (stillOpen()) {
          entries.value = res.entries; entry.value = null;
          draftSource.value = ""; draftFields.value = {}; draftBody.value = ""; changedOnDisk.value = false;
        }
      }
      lifecycle.value = null; resumeAction = "save";
    } catch (err) {
      recordFailure(err, branch);
      if (!needsChange.value) l.error = errorMessage(err);
      throw err;
    } finally { l.busy = false; }
  }
  /** After Create Change from a main-branch save: retry the same draft on the new branch. */
  async function resumeSave(branch: string): Promise<unknown> {
    needsChange.value = false;
    return resumeAction === "lifecycle" && lifecycle.value ? confirmLifecycle(branch) : saveDraft(branch);
  }
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
  return { projectId, collections, entries, selectedCollection, entry, draftSource, draftFields, draftBody, richBody, setBody, descriptor, saving, saveError, fieldErrors, conflict, needsChange, current, dirty, changedOnDisk, error, loading, open, refresh, selectCollection, selectEntry, canLeave, saveDraft, resumeSave, compare, reload, lifecycle, startLifecycle, cancelLifecycle, confirmLifecycle };
});
