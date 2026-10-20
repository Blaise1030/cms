import { defineStore } from "pinia";
import { computed, onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import { ApiError } from "@/core/api";
import type { ProjectSettings, SettingsDoc } from "@/core/types";
import { errorMessage } from "@orso/utils";

export interface SettingsConflict { reason: "stale_content" | "branch_changed"; message: string }
export type SettingsField = keyof ProjectSettings;

export const SETTINGS_FILE = "orso.json";
export const SETTINGS_FIELDS: readonly SettingsField[] = ["root", "installDir", "install", "dev"];
const isField = (key: string): key is SettingsField => (SETTINGS_FIELDS as readonly string[]).includes(key);

/** The four known fields as the file sets them; `""` means unset (a non-string value reads as unset). */
function draftOf(d: SettingsDoc | null): ProjectSettings {
  const value = (key: SettingsField) => { const v = d?.settings[key]; return typeof v === "string" ? v : ""; };
  return { root: value("root"), installDir: value("installDir"), install: value("install"), dev: value("dev") };
}

/** The repo-root `orso.json`: four known fields edited as strings, unknown keys kept as they are. */
export const useSettingsStore = defineStore("settings", () => {
  const daemon = useDaemon();
  const projectId = ref("");
  const doc = ref<SettingsDoc | null>(null);
  const draft = ref<ProjectSettings>(draftOf(null));
  const loading = ref(false);
  const saving = ref(false);
  const error = ref<string | null>(null);
  const conflict = ref<SettingsConflict | null>(null);
  /** The save was refused because the project is on main; waiting for a change to be created. */
  const needsChange = ref(false);
  /** The file changed on disk while the draft was being edited. */
  const changedOnDisk = ref(false);
  /** Per-field problems from the last refused save; they replace the file's own errors until cleared. */
  const saveErrors = ref<Record<string, string> | null>(null);
  let request = 0;

  const dirty = computed(() => {
    const d = doc.value;
    if (!d) return false;
    const base = draftOf(d);
    return SETTINGS_FIELDS.some(key => draft.value[key] !== base[key]);
  });
  const fieldErrors = computed<Record<string, string>>(() => saveErrors.value ?? doc.value?.errors ?? {});

  function clearSaveState() { error.value = null; conflict.value = null; needsChange.value = false; saveErrors.value = null; }
  function resetDraft(d: SettingsDoc | null) { draft.value = draftOf(d); }
  function apply(next: SettingsDoc) {
    doc.value = next; resetDraft(next); changedOnDisk.value = false; clearSaveState();
  }
  /** Map a refused read or write onto Create Change, conflict, field errors, or an error message; the draft stays. */
  function recordFailure(err: unknown) {
    if (err instanceof ApiError) {
      if (err.code === "CHANGE_REQUIRED") needsChange.value = true;
      else if (err.reason === "stale_content" || err.reason === "branch_changed") conflict.value = { reason: err.reason, message: err.message };
      else if (err.reason === "invalid_settings" && err.details?.length) {
        saveErrors.value = Object.fromEntries(err.details.map(d => [String(d.path[0] ?? "_file"), d.message]));
      }
      else error.value = err.message;
    } else error.value = errorMessage(err);
  }

  function setField(key: SettingsField, value: string) { draft.value = { ...draft.value, [key]: value }; }

  /** The file's keys in order with the draft's fields in place of the known ones; unset fields are left out. */
  function payload(d: SettingsDoc): Record<string, unknown> {
    const out: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(d.settings)) {
      if (!isField(key)) out[key] = value;
      else if (draft.value[key]) out[key] = draft.value[key];
    }
    for (const key of SETTINGS_FIELDS) if (draft.value[key] && !(key in out)) out[key] = draft.value[key];
    return out;
  }

  /** Re-read the file. A draft is never replaced; it is flagged when the file is no longer the one it started from. */
  async function reload() {
    const id = projectId.value; if (!id) return;
    const version = ++request;
    loading.value = true; error.value = null;
    try {
      const next = await daemon.api.settings(id);
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
    if (dirty.value && !window.confirm("Discard unsaved settings changes?")) return;
    const version = ++request;
    loading.value = true; error.value = null;
    try {
      const next = await daemon.api.settings(id);
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
    if (!window.confirm("Discard unsaved settings changes?")) return false;
    resetDraft(doc.value);
    return true;
  }
  /** Put the draft back to the loaded file; a file known to have changed on disk is re-read. */
  function discard() {
    const stale = changedOnDisk.value;
    resetDraft(doc.value); changedOnDisk.value = false; clearSaveState();
    if (stale) void reload();
  }

  /** Show `id`'s settings; reopening the same project re-reads them. */
  async function open(id: string) {
    if (id === projectId.value) return reload();
    if (!canLeave()) return;
    ++request; ++generation; saving.value = false;
    projectId.value = id; doc.value = null; resetDraft(null); changedOnDisk.value = false; clearSaveState();
    await reload();
  }

  /** Bumped when another project is opened; a save only records its outcome while its generation is current. */
  let generation = 0;
  let flight: { generation: number; promise: Promise<SettingsDoc> } | null = null;
  async function write(d: SettingsDoc, branch: string | undefined, gen: number): Promise<SettingsDoc> {
    const submitted = JSON.stringify(draft.value);
    saving.value = true; clearSaveState();
    try {
      const next = await daemon.api.saveSettings({
        project: projectId.value, branch: branch ?? d.branch, ...(d.exists ? { hash: d.hash } : {}), settings: payload(d),
      });
      if (gen !== generation) return next;
      // Edits typed while the request was in flight stay in the draft, rebased on the saved file.
      if (JSON.stringify(draft.value) === submitted) apply(next);
      else { doc.value = next; changedOnDisk.value = false; clearSaveState(); }
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
  async function save(branch?: string): Promise<SettingsDoc> {
    const d = doc.value;
    if (!d) throw new Error("No settings are open");
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
    daemon.on("fs.change", (e) => { if (e.projectId === projectId.value && e.path === SETTINGS_FILE) resync(); }),
    daemon.on("checkpoint.created", (e) => { if (e.projectId === projectId.value) resync(); }),
    daemon.on("events.lagged", resync),
    daemon.onReconnected(resync),
  ];
  onScopeDispose(() => offs.forEach(off => off()));

  return {
    projectId, doc, draft, dirty, loading, saving, error, conflict, needsChange, changedOnDisk, fieldErrors,
    open, setField, save, resumeSave, discard, reload, reloadFromDisk, refreshForBranch, canLeave,
  };
});
