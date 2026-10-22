import { defineStore } from "pinia";
import { computed, onScopeDispose, ref, shallowRef } from "vue";
import { useDaemon } from "@/core/daemon";
import { ApiError } from "@/core/api";
import type { ContentCollection, ContentEntry, ContentEntrySummary, ContentImpact, ContentVersion, FieldDescriptor } from "@/core/types";
import { errorMessage } from "@orso/utils";
import { diffDraft, cloneValue } from "./draft";
import { startingFields } from "./create";
import type { PendingImage } from "./ImageDialog.vue";

/** How a submitted entry names a pending image; the daemon swaps in the saved path. */
const IMAGE_TOKEN = "orso-image:";

export interface ContentConflict { reason: "branch_changed" | "stale_content"; message: string; branch?: string }
/** A new entry being named in the Create dialog. */
export interface ContentCreation { collection: string; title: string; slug: string; busy: boolean; error: string | null }
/** A rename or delete awaiting confirmation; `impact` is the daemon's dry run. */
export interface ContentLifecycle {
  action: "rename" | "delete"; collection: string; id: string;
  impact: ContentImpact | null; slug: string; busy: boolean; error: string | null;
}

/** File events within this window coalesce into one trailing refresh. */
export const REFRESH_DELAY_MS = 250;
const CONTENT_FILES = ["astro.config.mjs", "src/content.config.ts"];
const CONTENT_DIRS = ["src/content", "src/assets", "packages/astro"];
/** Whether a changed project path can change the catalog or an entry; "" is the watcher's overflow. */
export function affectsContent(path: string): boolean {
  return path === "" || CONTENT_FILES.includes(path)
    || CONTENT_DIRS.some(dir => path === dir || path.startsWith(`${dir}/`));
}

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
  /** Images inserted into the draft, shown from local preview URLs until a save uploads them. */
  const pendingImages = shallowRef<PendingImage[]>([]);
  /** Pending images the draft still uses; one the writer deleted again is not uploaded. */
  const usedImages = computed(() => {
    const text = draftBody.value + JSON.stringify(draftFields.value);
    return pendingImages.value.filter(i => text.includes(i.previewUrl));
  });
  function addImage(image: PendingImage) { pendingImages.value = [...pendingImages.value, image]; }
  /** Revoke the previews of `done` (default: all) and stop tracking them. */
  function releaseImages(done: PendingImage[] = pendingImages.value) {
    for (const image of done) URL.revokeObjectURL(image.previewUrl);
    pendingImages.value = pendingImages.value.filter(i => !done.includes(i));
  }
  /** Which mutation a Create Change resumes. */
  let resumeAction: "save" | "lifecycle" | "create" = "save";
  const openCollection = computed(() => collections.value.find(x => x.name === entry.value?.collection));
  /** The open entry's schema. A collection that is read only for now (preview not running) keeps it, so its draft stays a draft. */
  const descriptor = computed<FieldDescriptor | null>(() => {
    const c = openCollection.value;
    return c?.fields?.type === "object" ? c.fields : null;
  });
  /** Why the open entry cannot be saved right now, or null when it can. */
  const readOnlyReason = computed(() => {
    const c = openCollection.value;
    return !c || c.editable ? null : c.reason ?? "This collection is read only";
  });
  /** Markdown bodies get the rich editor; MDX and JSON stay raw. */
  const richBody = computed(() => entry.value?.kind === "markdown");
  /** Body text from the body editor; it only reports real edits, so opening an entry never dirties it. */
  function setBody(body: string) { draftBody.value = body; }
  /** Put an earlier version into the draft; nothing is written until Save. */
  function restoreVersion(v: ContentVersion) {
    draftFields.value = cloneValue(v.fields);
    if (entry.value?.kind !== "json") setBody(v.body);
  }
  const pending = computed(() => entry.value ? diffDraft(entry.value, draftFields.value, draftBody.value, descriptor.value) : null);
  const dirty = computed(() => !!entry.value && (draftSource.value !== entry.value.source || !!pending.value?.changed || usedImages.value.length > 0));
  function clearSaveState() { saveError.value = null; fieldErrors.value = {}; conflict.value = null; needsChange.value = false; current.value = null; }
  function loadEntry(next: ContentEntry) {
    releaseImages();
    targetBranch = null;
    // A freshly read entry belongs to the current branch.
    switchedTo = null;
    entry.value = next; draftSource.value = next.source;
    draftFields.value = cloneValue(next.fields ?? {}); draftBody.value = next.body ?? "";
    changedOnDisk.value = false; clearSaveState();
  }
  const changedOnDisk = ref(false);
  /**
   * The change a Create Change → resume moved the open draft to. The draft keeps its opened hash
   * (the change was cut from main), but later saves go to this change. Ordinary branch switches
   * never set it. Null means the opened entry's own branch.
   */
  let targetBranch: string | null = null;
  const boundBranch = (e: ContentEntry) => targetBranch ?? e.branch;
  const error = ref<string | null>(null);
  const loading = ref(false);
  let selectionRequest = 0;
  let refreshRequest = 0;
  let selectionPending = false;
  /** The branch the project switched to, until a refresh has applied the switch to the open entry. */
  let switchedTo: string | null = null;
  function canLeave() {
    if (!dirty.value) return true;
    if (!window.confirm("Discard unsaved content changes?")) return false;
    if (entry.value) loadEntry(entry.value);
    return true;
  }
  /** Whether the Content panel is mounted; file events refresh only while it is. */
  let active = false;
  let refreshTimer: ReturnType<typeof setTimeout> | null = null;
  function scheduleRefresh() {
    if (!active || !projectId.value) return;
    if (refreshTimer) clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => { refreshTimer = null; void refresh(); }, REFRESH_DELAY_MS);
  }
  /** The Content panel unmounted: stop following file events until it opens a project again. */
  function suspend() {
    active = false;
    if (refreshTimer) { clearTimeout(refreshTimer); refreshTimer = null; }
  }
  /** Show `id`; reopening the same project after a suspend refreshes it once. */
  /**
   * Reset the draft to the loaded entry, after confirming when pending images would be lost.
   * Not a fresh read: a Create Change target and an unapplied branch switch are kept, and a
   * stale entry is re-read by a refresh once it is clean.
   */
  function discard(): boolean {
    const e = entry.value; if (!e || !dirty.value) return true;
    const images = pendingImages.value.length;
    if (images && !window.confirm(`Discard your changes and ${images === 1 ? "the image" : `${images} images`} not saved yet?`)) return false;
    const stale = changedOnDisk.value, target = targetBranch, switched = switchedTo;
    loadEntry(e);
    targetBranch = target; switchedTo = switched;
    if (stale) void refresh();
    return true;
  }
  async function open(id: string) {
    const resumed = !active; active = true;
    if (id === projectId.value) { if (resumed) await refresh(); return; }
    if (!canLeave()) return;
    ++selectionRequest;
    ++refreshRequest;
    selectionPending = false;
    releaseImages();
    switchedTo = null;
    targetBranch = null;
    referenceEntries.value = {};
    projectId.value = id; selectedCollection.value = null; entry.value = null; lifecycle.value = null; entries.value = []; collections.value = [];
    draftSource.value = ""; draftFields.value = {}; draftBody.value = ""; changedOnDisk.value = false; clearSaveState();
    await refresh();
  }
  /** Entry ids per target collection for reference pickers, reloaded in place by each refresh. */
  const referenceEntries = ref<Record<string, string[]>>({});
  const referenceLoads = new Set<string>();
  async function fetchReferenceEntries(collection: string) {
    const id = projectId.value;
    if (!id || referenceLoads.has(collection)) return;
    referenceLoads.add(collection);
    try {
      const listed = await daemon.api.listContentEntries(id, collection);
      if (projectId.value === id) referenceEntries.value = { ...referenceEntries.value, [collection]: listed.map(e => e.id) };
    } catch { /* the picker keeps its last list or stays free text; the daemon checks references on save */ }
    finally { referenceLoads.delete(collection); }
  }
  async function loadReferenceEntries(collection: string) {
    if (!referenceEntries.value[collection]) await fetchReferenceEntries(collection);
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
      for (const target of Object.keys(referenceEntries.value)) void fetchReferenceEntries(target);
      // A refresh owns catalog loading; a user selection owns the detail view.
      if (selectionVersion !== selectionRequest) return;
      entries.value = nextEntries;
      // Any refresh that starts while a branch switch is unapplied applies it (fs events race it).
      const branch = switchedTo;
      if (openedEntry && canRefreshEntry && !selectionPending) {
        await reconcileEntry(id, branch, openedEntry, () => version === refreshRequest && selectionVersion === selectionRequest);
      } else if (!openedEntry && branch !== null && switchedTo === branch) switchedTo = null;
    } catch (e) { if (version === refreshRequest && selectionVersion === selectionRequest) error.value = errorMessage(e); }
    finally { if (version === refreshRequest) loading.value = false; }
  }
  /**
   * Re-read the open entry from disk. A clean entry reloads; after a branch switch it closes when
   * the branch does not have it. A draft is never discarded, replaced, or moved to another branch:
   * it keeps the branch and hash it was opened with (so Save conflicts elsewhere), and is flagged
   * while the file on disk is not the one it was opened from.
   */
  async function reconcileEntry(id: string, branch: string | null, opened: ContentEntry, current: () => boolean) {
    let next: ContentEntry | null = null;
    let missing: unknown = null;
    try { next = await daemon.api.contentEntry(id, opened.collection, opened.id); }
    catch (e) { if (!(e instanceof ApiError && e.code === "ENOENT")) throw e; missing = e; }
    if (!current() || entry.value !== opened) return;
    if (branch !== null && switchedTo === branch) switchedTo = null;
    if (dirty.value) changedOnDisk.value = !next || next.branch !== boundBranch(opened) || next.hash !== opened.hash;
    else if (next) loadEntry(next);
    else if (branch !== null) closeEntry();
    else throw missing;
  }
  /** Leave the detail view empty (no entry, no draft). */
  function closeEntry() {
    releaseImages(); targetBranch = null;
    entry.value = null; lifecycle.value = null; draftSource.value = ""; draftFields.value = {}; draftBody.value = ""; changedOnDisk.value = false; clearSaveState();
  }
  /** After the project switched to `branch` (change created or switched): reload Content from it. */
  async function refreshForBranch(branch: string) {
    if (!projectId.value) return;
    switchedTo = branch;
    await refresh();
  }
  async function selectCollection(name: string) {
    if (!canLeave()) return;
    const version = ++selectionRequest; selectionPending = true;
    releaseImages();
    selectedCollection.value = name; targetBranch = null; entry.value = null; lifecycle.value = null; draftSource.value = ""; draftFields.value = {}; draftBody.value = ""; entries.value = []; changedOnDisk.value = false; error.value = null; clearSaveState();
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
  /**
   * Send only the changed fields, plus the pending images the draft uses (their previews become
   * tokens). Throws after recording the failure so the draft and its pending images stay intact.
   */
  async function saveDraft(branch?: string): Promise<ContentEntry> {
    const e = entry.value; const change = pending.value;
    if (!e) throw new Error("No entry is open");
    if (!change?.changed) return e;
    const saved = { id: projectId.value, entryId: e.id };
    const submitted = { fields: JSON.stringify(draftFields.value), body: draftBody.value };
    const images = usedImages.value;
    const tokens = (text: string) => images.reduce((t, i) => t.split(i.previewUrl).join(IMAGE_TOKEN + i.clientId), text);
    saving.value = true; clearSaveState(); resumeAction = "save";
    try {
      const update = {
        project: saved.id, collection: e.collection, id: e.id, branch: branch ?? boundBranch(e), hash: e.hash,
        fields: images.length ? JSON.parse(tokens(JSON.stringify(change.fields))) as Record<string, unknown> : change.fields,
        ...(change.body !== undefined ? { body: tokens(change.body) } : {}), ...(change.unset.length ? { unset: change.unset } : {}),
      };
      const uploads = images.map(({ clientId, alt, filename, file }) => ({ clientId, alt, filename, file }));
      const { images: placed, ...next } = await (uploads.length ? daemon.api.updateContentEntry(update, uploads) : daemon.api.updateContentEntry(update));
      if (projectId.value === saved.id && entry.value?.id === saved.entryId) {
        // Edits typed while the request was in flight stay in the draft, rebased on the saved entry;
        // the uploaded images' previews in it become their saved links.
        if (JSON.stringify(draftFields.value) === submitted.fields && draftBody.value === submitted.body) loadEntry(next);
        else {
          const links = (text: string) => images.reduce((t, i) => placed?.[i.clientId] ? t.split(i.previewUrl).join(placed[i.clientId]!.link) : t, text);
          draftBody.value = links(draftBody.value);
          draftFields.value = JSON.parse(links(JSON.stringify(draftFields.value))) as Record<string, unknown>;
          releaseImages(images);
          entry.value = next; targetBranch = null; draftSource.value = next.source; changedOnDisk.value = false; clearSaveState();
        }
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
    const target = { project: saved.id, collection: e.collection, id: e.id, branch: branch ?? boundBranch(e), hash: e.hash };
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
          entries.value = res.entries; closeEntry();
        }
      }
      lifecycle.value = null; resumeAction = "save";
    } catch (err) {
      recordFailure(err, branch);
      if (!needsChange.value) l.error = errorMessage(err);
      throw err;
    } finally { l.busy = false; }
  }
  const creation = ref<ContentCreation | null>(null);
  /** Open the Create dialog for a multi-entry collection; an open draft must be left first. */
  function startCreate(collection: string) {
    if (!canLeave()) return;
    clearSaveState();
    creation.value = { collection, title: "", slug: "", busy: false, error: null };
  }
  function cancelCreate() { if (creation.value?.busy) return; creation.value = null; clearSaveState(); }
  /**
   * Create the named entry with schema defaults on `branch`, then open it. On main this records
   * `needsChange` so Create Change can resume it. Throws after recording the failure.
   */
  async function confirmCreate(branch: string): Promise<ContentEntry | undefined> {
    const c = creation.value; const id = projectId.value;
    const collection = collections.value.find(x => x.name === c?.collection);
    if (!c || c.busy || !collection?.fields) return;
    const start = startingFields(collection.fields, c.title.trim());
    if ("reason" in start) { c.error = start.reason; return; }
    resumeAction = "create";
    c.busy = true; c.error = null; clearSaveState();
    try {
      const saved = await daemon.api.createContentEntry({
        project: id, collection: c.collection, slug: c.slug.trim(), branch, fields: start.fields,
        ...(collection.kind !== "json" ? { body: "" } : {}),
      });
      resumeAction = "save";
      if (projectId.value !== id) return saved;
      creation.value = null;
      ++selectionRequest; selectionPending = false;
      selectedCollection.value = saved.collection; lifecycle.value = null; loadEntry(saved);
      try { entries.value = await daemon.api.listContentEntries(id, saved.collection); } catch { /* the next refresh lists it */ }
      return saved;
    } catch (err) {
      recordFailure(err, branch);
      if (!needsChange.value) c.error = errorMessage(err);
      throw err;
    } finally { c.busy = false; }
  }
  /** After Create Change from main: retry the same save, rename/delete, or create on the new branch. */
  async function resumeSave(branch: string): Promise<unknown> {
    needsChange.value = false;
    if (resumeAction === "create" && creation.value) return confirmCreate(branch);
    // The writer created this change for the open draft: it stays the target even if this try fails.
    if (entry.value) { targetBranch = branch; changedOnDisk.value = false; }
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
    daemon.on("fs.change", (e) => { if (e.projectId === projectId.value && affectsContent(e.path)) scheduleRefresh(); }),
    // Saving needs a ready preview, so any change in its state can change what is editable.
    daemon.on("devserver.status", (e) => { if (e.projectId === projectId.value) scheduleRefresh(); }),
    // Agent ctl / turn-end: re-fetch collections, entry lists, and the open entry now.
    daemon.on("content.refresh", (e) => { if (e.projectId === projectId.value) void refresh(); }),
    daemon.on("events.lagged", scheduleRefresh),
    daemon.onReconnected(scheduleRefresh),
  ];
  onScopeDispose(() => { suspend(); offs.forEach(off => off()); });
  return { discard, suspend, referenceEntries, loadReferenceEntries, pendingImages, addImage, projectId, collections, entries, selectedCollection, entry, draftSource, draftFields, draftBody, richBody, setBody, restoreVersion, descriptor, readOnlyReason, saving, saveError, fieldErrors, conflict, needsChange, current, dirty, changedOnDisk, error, loading, open, refresh, refreshForBranch, selectCollection, selectEntry, canLeave, saveDraft, resumeSave, compare, reload, lifecycle, startLifecycle, cancelLifecycle, confirmLifecycle, creation, startCreate, cancelCreate, confirmCreate };
});
