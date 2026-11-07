import { defineStore } from "pinia";
import { computed, onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import { ApiError } from "@/core/api";
import type { CatalogItem, SkillDoc, SkillMeta, SkillScope, SkillSource } from "@/core/types";
import { errorMessage } from "@orso/utils";

export interface SkillConflict { reason: "stale_content" | "branch_changed"; message: string }

/** The selected skill, keyed by `(id, scope)` so the same id can live in both scopes. */
export interface SkillSelection { id: string; scope: SkillScope }

/** The project's installed skills plus the built-in catalog, edited one skill at a time. */
export const useSkillsStore = defineStore("skills", () => {
  const daemon = useDaemon();
  const projectId = ref("");
  /** The active change's branch the editor opened on; project-scope mutations go to it. */
  const branch = ref<string | null>(null);
  const list = ref<SkillMeta[]>([]);
  const catalog = ref<CatalogItem[]>([]);
  const selected = ref<SkillSelection | null>(null);
  const doc = ref<SkillDoc | null>(null);
  const draftName = ref("");
  const draftDescription = ref("");
  const draftBody = ref("");
  /** A brand-new skill is being authored in the editor; nothing is on disk yet. */
  const creating = ref(false);
  const newScope = ref<SkillScope>("project");
  const loading = ref(false);
  const saving = ref(false);
  const error = ref<string | null>(null);
  const conflict = ref<SkillConflict | null>(null);
  /** The save was refused because the project is on main; waiting for a change to be created. */
  const needsChange = ref(false);
  /** The mutation to retry once Create Change finishes. */
  const pendingAction = ref<"save" | "uninstall" | "create" | null>(null);
  /** The skill changed on disk while the draft was being edited. */
  const changedOnDisk = ref(false);
  /** The daemon could not read the skill's front matter; the message says why. */
  const invalid = ref<string | null>(null);
  /** The skill's raw text when it is invalid, shown read-only. */
  const invalidSource = ref<string | null>(null);
  let request = 0;

  const dirty = computed(() => {
    if (creating.value) {
      return !!(draftName.value.trim() || draftDescription.value.trim() || draftBody.value.trim());
    }
    const d = doc.value;
    if (!d) return false;
    return draftName.value !== d.name || draftDescription.value !== d.description || draftBody.value !== d.body;
  });

  function clearSaveState() {
    error.value = null; conflict.value = null; needsChange.value = false; invalid.value = null; invalidSource.value = null; pendingAction.value = null;
  }
  function resetDrafts(d: SkillDoc | null) {
    draftName.value = d?.name ?? ""; draftDescription.value = d?.description ?? ""; draftBody.value = d?.body ?? "";
  }
  function apply(next: SkillDoc) {
    doc.value = next; resetDrafts(next); changedOnDisk.value = false; clearSaveState();
  }
  /** Map a refused read or write onto Create Change, conflict, or an error message; the draft stays. */
  function recordFailure(err: unknown, scope: SkillScope, opts: { invalidAsError?: boolean } = {}) {
    if (err instanceof ApiError) {
      if (scope === "project" && err.code === "CHANGE_REQUIRED") needsChange.value = true;
      else if (err.reason === "stale_content" || err.reason === "branch_changed") conflict.value = { reason: err.reason, message: err.message };
      else if (!opts.invalidAsError && err.reason === "invalid_skill") {
        invalid.value = err.message;
        invalidSource.value = err.details?.find(d => d.path[0] === "source")?.message ?? null;
      }
      else error.value = err.message;
    } else error.value = errorMessage(err);
  }

  function setName(name: string) { draftName.value = name; }
  function setDescription(description: string) { draftDescription.value = description; }
  function setBody(body: string) { draftBody.value = body; }

  /** Start a brand-new skill in the outer editor; it is not written until `create`. */
  function startNew(scope: SkillScope): boolean {
    if (!projectId.value) return false;
    if (!canLeave()) return false;
    ++request; ++generation; saving.value = false;
    selected.value = null; doc.value = null; resetDrafts(null); changedOnDisk.value = false; clearSaveState();
    creating.value = true;
    newScope.value = scope;
    return true;
  }

  function cancelNew() {
    creating.value = false;
    resetDrafts(null);
    clearSaveState();
  }

  /** Re-read the list (and the open skill). A draft is never replaced; it is flagged when the file is no longer the one it started from. */
  async function reload() {
    const id = projectId.value; if (!id) return;
    const version = ++request;
    loading.value = true; error.value = null;
    try {
      const next = await daemon.api.skills(id);
      if (version !== request) return;
      list.value = next.installed;
      catalog.value = next.catalog;
      const sel = selected.value;
      if (!sel) return;
      if (!next.installed.some(m => m.id === sel.id && m.scope === sel.scope)) {
        // The skill vanished from the list; a dirty draft is kept and flagged rather than discarded.
        if (dirty.value) { changedOnDisk.value = true; return; }
        selected.value = null; doc.value = null; resetDrafts(null); return;
      }
      await readDoc(sel, version);
    } catch (err) {
      if (version === request) recordFailure(err, selected.value?.scope ?? "project");
    } finally { if (version === request) loading.value = false; }
  }

  /** Re-read the open skill from disk, reconciling a clean draft and flagging a dirty one. */
  async function readDoc(sel: SkillSelection, version: number) {
    const next = await daemon.api.skill(projectId.value, sel.id, sel.scope);
    if (version !== request) return;
    if (!dirty.value) apply(next);
    else if (next.hash !== doc.value?.hash) changedOnDisk.value = true;
  }

  /** Re-read `id`'s active change branch, unless another project was opened meanwhile. */
  async function readBranch(id: string) {
    try {
      const change = await daemon.api.activeChange(id);
      if (projectId.value === id) branch.value = change.branch;
    } catch {
      if (projectId.value === id) branch.value = null;
    }
  }

  /** Show `id`'s skills; reopening the same project re-reads them and its active branch. */
  async function open(id: string) {
    if (id === projectId.value) {
      await readBranch(id);
      return reload();
    }
    if (!canLeave()) return;
    ++request; ++generation; saving.value = false;
    projectId.value = id; branch.value = null; list.value = []; catalog.value = [];
    selected.value = null; doc.value = null; resetDrafts(null); changedOnDisk.value = false; clearSaveState();
    creating.value = false;
    await readBranch(id);
    await reload();
  }

  /** The project moved to `branch`: a clean draft is re-read from it; a dirty one is kept and flagged. */
  async function refreshForBranch(next: string) {
    branch.value = next;
    if (!projectId.value) return;
    if (dirty.value) { changedOnDisk.value = true; return; }
    await reload();
  }

  /** Select a skill to edit, loading its body. */
  async function select(id: string, scope: SkillScope) {
    // Creating is an internal transition (install selects its own result); no discard prompt.
    if (!creating.value && !canLeave()) return;
    const version = ++request; ++generation; saving.value = false;
    selected.value = { id, scope };
    doc.value = null; resetDrafts(null); changedOnDisk.value = false; clearSaveState();
    loading.value = true; error.value = null;
    try {
      const next = await daemon.api.skill(projectId.value, id, scope);
      if (version !== request) return;
      apply(next);
    } catch (err) {
      if (version === request) recordFailure(err, scope);
    } finally { if (version === request) loading.value = false; }
  }

  function canLeave() {
    if (creating.value) {
      if (!dirty.value) { creating.value = false; resetDrafts(null); return true; }
      if (!window.confirm("Discard this new skill?")) return false;
      creating.value = false;
      resetDrafts(null);
      clearSaveState();
      return true;
    }
    if (!dirty.value) return true;
    if (!window.confirm("Discard unsaved skill changes?")) return false;
    resetDrafts(doc.value);
    return true;
  }
  /** Put the draft back to the loaded file; a file known to have changed on disk is re-read. */
  function discard() {
    if (creating.value) { cancelNew(); return; }
    const stale = changedOnDisk.value;
    resetDrafts(doc.value); changedOnDisk.value = false; clearSaveState();
    if (stale) void reload();
  }

  /** The writer chose to take the file as it is on disk: after confirming, it replaces the draft. */
  async function reloadFromDisk() {
    const sel = selected.value;
    if (!sel || !projectId.value) return;
    if (dirty.value && !window.confirm("Discard unsaved skill changes?")) return;
    const version = ++request;
    loading.value = true; error.value = null;
    try {
      const next = await daemon.api.skill(projectId.value, sel.id, sel.scope);
      if (version === request) apply(next);
    } catch (err) {
      if (version === request) recordFailure(err, sel.scope);
    } finally { if (version === request) loading.value = false; }
  }

  /** Bumped when another skill (or project) is selected; a save only records its outcome while its generation is current. */
  let generation = 0;
  let flight: { generation: number; id: string; scope: SkillScope; promise: Promise<SkillDoc> } | null = null;
  async function write(d: SkillDoc, gen: number): Promise<SkillDoc> {
    const submitted = { name: draftName.value, description: draftDescription.value, body: draftBody.value };
    saving.value = true; clearSaveState();
    try {
      const next = await daemon.api.saveSkill(d.id, {
        ...(d.scope === "project" ? { project: projectId.value, branch: branch.value ?? "main" } : {}),
        scope: d.scope, name: draftName.value, description: draftDescription.value, body: draftBody.value, hash: d.hash,
      });
      if (gen !== generation) return next;
      // A global rename shows in the sidebar immediately, without waiting for a reload.
      const entry = list.value.find(m => m.id === next.id && m.scope === next.scope);
      if (entry) { entry.name = next.name; entry.description = next.description; entry.path = next.path; }
      // Edits typed while the request was in flight stay in the draft, rebased on the saved file.
      if (draftName.value === submitted.name && draftDescription.value === submitted.description && draftBody.value === submitted.body) apply(next);
      else { doc.value = next; changedOnDisk.value = false; clearSaveState(); }
      return next;
    } catch (err) {
      if (gen === generation) {
        recordFailure(err, d.scope, { invalidAsError: true });
        if (needsChange.value) pendingAction.value = "save";
      }
      throw err;
    } finally { if (gen === generation) saving.value = false; }
  }
  /**
   * Write the draft. Project saves need the active change; global saves never do. A save already in
   * flight for this skill is shared, not repeated. Throws after recording the failure, the draft kept.
   */
  async function save(): Promise<SkillDoc> {
    if (creating.value) {
      await create();
      const saved = doc.value;
      if (!saved) throw new Error("The new skill could not be opened");
      return saved;
    }
    const d = doc.value;
    if (!d) throw new Error("No skill is open");
    if (flight?.generation === generation) return flight.promise;
    if (!dirty.value) return d;
    const mine = { generation, id: d.id, scope: d.scope, promise: write(d, generation) };
    flight = mine;
    try { return await mine.promise; } finally { if (flight === mine) flight = null; }
  }
  /** Write the new skill: install a blank package, then save the instructions typed in the editor. */
  async function create(): Promise<SkillMeta> {
    if (!creating.value) throw new Error("No new skill is open");
    const name = draftName.value.trim();
    const description = draftDescription.value.trim();
    if (!name || !description) throw new Error("Name and description are required");
    const body = draftBody.value;
    const scope = newScope.value;
    try {
      const meta = await install(scope, { blank: { name, description } });
      creating.value = false;
      if (body.trim()) {
        setBody(body);
        await save();
      }
      return meta;
    } catch (err) {
      if (needsChange.value) pendingAction.value = "create";
      throw err;
    }
  }
  /** After Create Change from main: retry the refused mutation (save, create, or uninstall). */
  function resumeSave(nextBranch: string) {
    needsChange.value = false;
    branch.value = nextBranch;
    const action = pendingAction.value;
    pendingAction.value = null;
    if (action === "uninstall") return uninstall();
    if (action === "create") return create();
    if (action === "save") return save();
  }

  /** Install a skill into a scope, then refresh the list and select it. */
  async function install(scope: SkillScope, source: SkillSource): Promise<SkillMeta> {
    saving.value = true; clearSaveState();
    try {
      const meta = await daemon.api.installSkill({
        ...(scope === "project" ? { project: projectId.value, branch: branch.value ?? "main" } : {}),
        scope, source,
      });
      await reload();
      await select(meta.id, meta.scope);
      return meta;
    } catch (err) {
      recordFailure(err, scope, { invalidAsError: true });
      throw err;
    } finally { saving.value = false; }
  }
  /** Remove the selected skill, then clear it and refresh the list. */
  async function uninstall() {
    const sel = selected.value;
    if (!sel) return;
    saving.value = true; clearSaveState();
    try {
      await daemon.api.uninstallSkill(sel.id, sel.scope === "project"
        ? { project: projectId.value, scope: sel.scope, branch: branch.value ?? "main" }
        : { scope: sel.scope });
      selected.value = null; doc.value = null; resetDrafts(null);
      await reload();
    } catch (err) {
      recordFailure(err, sel.scope);
      if (needsChange.value) pendingAction.value = "uninstall";
      throw err;
    } finally { saving.value = false; }
  }

  const resync = () => { void reload(); };
  const offs = [
    daemon.on("checkpoint.created", (e) => { if (e.projectId === projectId.value) resync(); }),
    daemon.on("events.lagged", resync),
    daemon.onReconnected(resync),
  ];
  onScopeDispose(() => offs.forEach(off => off()));

  return {
    projectId, branch, list, catalog, selected, doc, draftName, draftDescription, draftBody, creating, newScope,
    dirty, loading, saving, error, conflict, needsChange, changedOnDisk, invalid, invalidSource,
    open, select, setName, setDescription, setBody, startNew, cancelNew, create, save, resumeSave, discard,
    reloadFromDisk, install, uninstall, reload, refreshForBranch, canLeave,
  };
});
