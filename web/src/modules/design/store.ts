import { defineStore } from "pinia";
import { computed, onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import { ApiError } from "@/core/api";
import type { DesignDoc, DesignTokens } from "@/core/types";
import { errorMessage } from "@workbench/utils";
import { cloneValue, sameValue } from "@/modules/content/draft";

export interface DesignConflict { reason: "stale_content" | "branch_changed"; message: string }

/** The project's root DESIGN.md: its tokens (frontmatter) and Markdown body, edited as one draft. */
export const useDesignStore = defineStore("design", () => {
  const daemon = useDaemon();
  const projectId = ref("");
  const doc = ref<DesignDoc | null>(null);
  const draftTokens = ref<DesignTokens>({});
  const draftBody = ref("");
  /** The file does not exist yet and the writer chose to start it from the starter. */
  const created = ref(false);
  const loading = ref(false);
  const saving = ref(false);
  const error = ref<string | null>(null);
  const conflict = ref<DesignConflict | null>(null);
  /** The save was refused because the project is on main; waiting for a change to be created. */
  const needsChange = ref(false);
  /** The file changed on disk while the draft was being edited. */
  const changedOnDisk = ref(false);
  /** The daemon could not read the file's frontmatter; the message says why. */
  const invalid = ref<string | null>(null);
  /** The file's raw text when it is invalid, shown read-only. */
  const invalidSource = ref<string | null>(null);
  let request = 0;

  const dirty = computed(() => {
    const d = doc.value;
    if (!d) return false;
    if (!d.exists) return created.value;
    return !sameValue(draftTokens.value, d.tokens) || draftBody.value !== d.body;
  });

  function clearSaveState() { error.value = null; conflict.value = null; needsChange.value = false; invalid.value = null; invalidSource.value = null; }
  function resetDrafts(d: DesignDoc | null) {
    draftTokens.value = cloneValue(d?.tokens ?? {}); draftBody.value = d?.body ?? ""; created.value = false;
  }
  function apply(next: DesignDoc) {
    doc.value = next; resetDrafts(next); changedOnDisk.value = false; clearSaveState();
  }
  /** Map a refused read or write onto Create Change, conflict, or an error message; the draft stays. */
  function recordFailure(err: unknown) {
    if (err instanceof ApiError) {
      if (err.code === "CHANGE_REQUIRED") needsChange.value = true;
      else if (err.reason === "stale_content" || err.reason === "branch_changed") conflict.value = { reason: err.reason, message: err.message };
      else if (err.reason === "invalid_design") {
        invalid.value = err.message;
        invalidSource.value = err.details?.find(d => d.path[0] === "source")?.message ?? null;
      }
      else error.value = err.message;
    } else error.value = errorMessage(err);
  }

  function setTokens(tokens: DesignTokens) { draftTokens.value = tokens; }
  function setBody(body: string) { draftBody.value = body; }
  /** Start a missing file from the daemon's starter. */
  function create() {
    const starter = doc.value?.starter;
    if (!doc.value || doc.value.exists || !starter) return;
    draftTokens.value = cloneValue(starter.tokens); draftBody.value = starter.body; created.value = true;
  }

  /** Re-read the file. A draft is never replaced; it is flagged when the file is no longer the one it started from. */
  async function reload() {
    const id = projectId.value; if (!id) return;
    const version = ++request;
    loading.value = true; error.value = null; invalid.value = null; invalidSource.value = null;
    try {
      const next = await daemon.api.design(id);
      if (version !== request) return;
      if (!dirty.value) apply(next);
      else if (next.hash !== doc.value?.hash) changedOnDisk.value = true;
    } catch (err) {
      if (version === request) recordFailure(err);
    } finally { if (version === request) loading.value = false; }
  }

  /** The writer chose to take the file as it is on disk: after confirming, it replaces the draft. */
  async function reloadFromDisk() {
    const id = projectId.value; if (!id) return;
    if (dirty.value && !window.confirm("Discard unsaved design changes?")) return;
    const version = ++request;
    loading.value = true; error.value = null;
    try {
      const next = await daemon.api.design(id);
      if (version === request) apply(next);
    } catch (err) {
      if (version === request) recordFailure(err);
    } finally { if (version === request) loading.value = false; }
  }

  /** The project moved to `branch`: a clean draft is re-read from it; a dirty one is kept and flagged. */
  async function refreshForBranch(_branch: string) {
    if (!projectId.value) return;
    if (dirty.value) { changedOnDisk.value = true; return; }
    await reload();
  }

  function canLeave() {
    if (!dirty.value) return true;
    if (!window.confirm("Discard unsaved design changes?")) return false;
    resetDrafts(doc.value);
    return true;
  }
  /** Put the draft back to the loaded file; a file known to have changed on disk is re-read. */
  function discard() {
    const stale = changedOnDisk.value;
    resetDrafts(doc.value); changedOnDisk.value = false; clearSaveState();
    if (stale) void reload();
  }

  /** Show `id`'s design; reopening the same project re-reads it. */
  async function open(id: string) {
    if (id === projectId.value) return reload();
    if (!canLeave()) return;
    ++request; ++generation; saving.value = false;
    projectId.value = id; doc.value = null; resetDrafts(null); changedOnDisk.value = false; clearSaveState();
    await reload();
  }

  /** Bumped when another project is opened; a save only records its outcome while its generation is current. */
  let generation = 0;
  let flight: { generation: number; promise: Promise<DesignDoc> } | null = null;
  async function write(d: DesignDoc, branch: string | undefined, gen: number): Promise<DesignDoc> {
    const submitted = { tokens: JSON.stringify(draftTokens.value), body: draftBody.value };
    saving.value = true; clearSaveState();
    try {
      const next = await daemon.api.saveDesign({
        project: projectId.value, branch: branch ?? d.branch, ...(d.exists ? { hash: d.hash } : {}),
        tokens: cloneValue(draftTokens.value), body: draftBody.value,
      });
      if (gen !== generation) return next;
      // Edits typed while the request was in flight stay in the draft, rebased on the saved file.
      if (JSON.stringify(draftTokens.value) === submitted.tokens && draftBody.value === submitted.body) apply(next);
      else { doc.value = next; created.value = false; changedOnDisk.value = false; clearSaveState(); }
      return next;
    } catch (err) {
      if (gen === generation) recordFailure(err);
      throw err;
    } finally { if (gen === generation) saving.value = false; }
  }
  /**
   * Write the draft to `branch ?? doc.branch`; a new file omits the hash. A save already in flight
   * for this project is shared, not repeated. Throws after recording the failure, the draft kept.
   */
  async function save(branch?: string): Promise<DesignDoc> {
    const d = doc.value;
    if (!d) throw new Error("No design is open");
    if (flight?.generation === generation) return flight.promise;
    if (!dirty.value) return d;
    const mine = { generation, promise: write(d, branch, generation) };
    flight = mine;
    try { return await mine.promise; } finally { if (flight === mine) flight = null; }
  }
  /** After Create Change from main: retry the same save on the new branch. */
  function resumeSave(branch: string) {
    needsChange.value = false;
    return save(branch);
  }

  const resync = () => { void reload(); };
  const offs = [
    daemon.on("checkpoint.created", (e) => { if (e.projectId === projectId.value) resync(); }),
    daemon.on("events.lagged", resync),
    daemon.onReconnected(resync),
  ];
  onScopeDispose(() => offs.forEach(off => off()));

  return {
    projectId, doc, draftTokens, draftBody, dirty, loading, saving, error, conflict, needsChange, changedOnDisk, invalid, invalidSource,
    open, create, setTokens, setBody, save, resumeSave, discard, reload, reloadFromDisk, refreshForBranch, canLeave,
  };
});
