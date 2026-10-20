import { defineStore } from "pinia";
import { ref } from "vue";
import { ApiError } from "@/core/api";
import { useDaemon } from "@/core/daemon";
import type { AutoResolveStatus, Change, ChangesUpdate, ConflictCandidate, ConflictChoice, ConflictSession, PrStatus, ProtectionHint } from "@/core/types";
import { errorMessage } from "@orso/utils";
import { useFilesStore } from "@/modules/files/store";
import { usePreviewStore } from "@/modules/preview/store";
import { useContentStore } from "@/modules/content/store";
import { useDesignStore } from "@/modules/design/store";
import { useSettingsStore } from "@/modules/settings/store";
import { useAgentProvidersStore } from "@/modules/chat/providers";
import { getLatest as syncAgain } from "./getLatest";

const SYNC_UNAVAILABLE = "Your AI assistant isn't available.";
const SYNC_START_FAILED = "Couldn't start combining. Please try again.";
const SYNC_REQUEST_FAILED = "Something went wrong. Please try again.";
const SYNC_DRAFT_MOVED = "Your draft changed while combining. Please sync again.";

/** How often the active change's PR status is re-read while it has a PR. */
export const STATUS_POLL_MS = 20_000;

export const useChangesStore = defineStore("changes", () => {
  const daemon = useDaemon();

  const projectId = ref<string | null>(null);
  const active = ref<Change | null>(null);
  const items = ref<Change[]>([]);
  const prUrl = ref<string | null>(null);
  /** "Protect main" hint from the last share; null when hidden. */
  const protectionHint = ref<ProtectionHint | null>(null);
  const creating = ref(false);
  const sharing = ref(false);
  const markingReady = ref(false);
  const switching = ref(false);
  const updating = ref(false);
  const cleaningUp = ref(false);
  const listing = ref(false);
  const loading = ref(false);
  const error = ref<string | null>(null);
  /** Share failed with expired/missing auth — show a sign-in prompt. */
  const needsSignIn = ref(false);
  /** PR state / checks / preview URL of the active change; null without a PR. */
  const status = ref<PrStatus | null>(null);
  const conflictSessions = ref<ConflictSession[]>([]);
  const selectedConflictId = ref<string | null>(null);
  const currentConflict = ref<ConflictSession | null>(null);
  const candidate = ref<ConflictCandidate | null>(null);
  const conflictLoading = ref(false);
  const conflictError = ref<string | null>(null);
  /** The conflict session the auto-resolve dialog shows; null when closed. */
  const syncSessionId = ref<string | null>(null);
  const syncStatus = ref<AutoResolveStatus | null>(null);
  let pollTimer: ReturnType<typeof setTimeout> | null = null;
  /** Bumped by every stop/refresh: a refresh that finds it changed is stale and neither
   *  applies its result nor schedules a poll, so at most one timer ever exists. */
  let statusRun = 0;

  function stopStatusPolling() {
    statusRun++;
    if (pollTimer !== null) clearTimeout(pollTimer);
    pollTimer = null;
  }

  /**
   * Re-read the active change's PR status, then poll again only while a PR exists.
   * Failures keep the last known status (GitHub may be briefly unreachable).
   */
  async function refreshStatus() {
    stopStatusPolling();
    const run = statusRun;
    const id = projectId.value;
    const branch = active.value?.branch;
    if (!id || !branch) {
      status.value = null;
      return;
    }
    try {
      const pr = await daemon.api.changeStatus(id, branch);
      if (run !== statusRun) return;
      status.value = pr;
      if (pr) prUrl.value = pr.url;
    } catch {
      if (run !== statusRun) return;
    }
    if (status.value) {
      pollTimer = setTimeout(() => void refreshStatus(), STATUS_POLL_MS);
    }
  }

  /** Refresh every view after the working tree moved; `branch` when the project switched to it. */
  async function refreshTreeAndPreview(branch?: string) {
    await Promise.all([
      useFilesStore().refresh(),
      branch ? useContentStore().refreshForBranch(branch) : undefined,
      branch ? useDesignStore().refreshForBranch(branch) : undefined,
      branch ? useSettingsStore().refreshForBranch(branch) : undefined,
    ]);
    usePreviewStore().reload();
  }

  async function load(id: string) {
    if (projectId.value !== id) {
      stopStatusPolling();
      active.value = null;
      prUrl.value = null;
      status.value = null;
      protectionHint.value = null;
      items.value = [];
      error.value = null;
      needsSignIn.value = false;
      conflictSessions.value = [];
      selectedConflictId.value = null;
      currentConflict.value = null;
      candidate.value = null;
      syncSessionId.value = null;
      syncStatus.value = null;
    }
    projectId.value = id;
    loading.value = true;
    error.value = null;
    try {
      const change = await daemon.api.activeChange(id);
      if (projectId.value !== id) return;
      active.value = change;
      if (change.prUrl) prUrl.value = change.prUrl;
    } catch (e) {
      if (projectId.value !== id) return;
      active.value = null;
      prUrl.value = null;
      status.value = null;
      stopStatusPolling();
      if (e instanceof ApiError && e.code === "ENOENT") {
        error.value = null;
      } else {
        error.value = errorMessage(e);
      }
    } finally {
      if (projectId.value === id) loading.value = false;
    }
    if (projectId.value === id && active.value) await refreshStatus();
  }

  async function list(id: string) {
    projectId.value = id;
    listing.value = true;
    error.value = null;
    try {
      items.value = await daemon.api.listChanges(id);
    } catch (e) {
      items.value = [];
      error.value = errorMessage(e);
      throw e;
    } finally {
      listing.value = false;
    }
  }

  async function create(id: string, title: string): Promise<Change> {
    const trimmed = title.trim();
    if (!trimmed) throw new Error("Change title is required");
    creating.value = true;
    error.value = null;
    needsSignIn.value = false;
    try {
      const change = await daemon.api.createChange(id, trimmed);
      projectId.value = id;
      active.value = change;
      prUrl.value = null;
      status.value = null;
      stopStatusPolling();
      protectionHint.value = null;
      await refreshTreeAndPreview(change.branch);
      return change;
    } catch (e) {
      error.value = errorMessage(e);
      throw e;
    } finally {
      creating.value = false;
    }
  }

  async function switchTo(id: string, branch: string): Promise<Change> {
    // The old change's poll must not fire mid-switch.
    stopStatusPolling();
    switching.value = true;
    error.value = null;
    try {
      const change = await daemon.api.switchChange(id, branch);
      projectId.value = id;
      active.value = change;
      prUrl.value = change.prUrl ?? null;
      status.value = null;
      protectionHint.value = null;
      await refreshTreeAndPreview(change.branch);
      await refreshStatus();
      return change;
    } catch (e) {
      error.value = errorMessage(e);
      throw e;
    } finally {
      switching.value = false;
    }
  }

  /** Refresh every view after a Get latest (ours, or one the daemon finished later). */
  async function applyUpdate(id: string, result: ChangesUpdate) {
    await refreshTreeAndPreview();
    // Branch tips (and so "last edited") moved; a failed list refresh is not an update failure.
    await Promise.all([load(id), list(id).catch(() => undefined)]);
    const first = result.conflicts.find((c) => result.conflictSessions?.[c.branch]);
    if (first) await startSync(id, result.conflictSessions![first.branch]!);
  }

  function syncFailed(sessionId: string, message: string) {
    syncStatus.value = { sessionId, state: "failed", stage: "", questions: [], summary: null, error: message };
  }

  /** Let the user's AI assistant combine the conflict session in the background. */
  async function startSync(id: string, sessionId: string) {
    syncSessionId.value = sessionId;
    syncStatus.value = { sessionId, state: "preparing", stage: "", questions: [], summary: null, error: null };
    const provider = useAgentProvidersStore().provider;
    if (!provider) {
      syncFailed(sessionId, SYNC_UNAVAILABLE);
      return;
    }
    try {
      await daemon.api.startAutoResolve(id, sessionId, provider);
    } catch (e) {
      if (syncSessionId.value !== sessionId) return;
      syncFailed(sessionId, e instanceof ApiError && e.status === 400 ? SYNC_UNAVAILABLE : SYNC_START_FAILED);
    }
  }

  /** A failed answer/stop shows on the open status; once done the job is gone (404), so ignore it.
   *  Daemon text is never shown: it talks about git. */
  function syncRequestFailed(sessionId: string) {
    const current = syncStatus.value;
    if (syncSessionId.value !== sessionId || !current || current.state === "done") return;
    syncStatus.value = { ...current, error: SYNC_REQUEST_FAILED };
  }

  async function answerSync(id: string, answers: Record<string, string>) {
    const sessionId = syncSessionId.value;
    if (!sessionId) return;
    try {
      await daemon.api.answerAutoResolve(id, sessionId, answers);
    } catch {
      syncRequestFailed(sessionId);
    }
  }

  async function stopSync(id: string) {
    const sessionId = syncSessionId.value;
    if (!sessionId) return;
    try {
      await daemon.api.cancelAutoResolve(id, sessionId);
    } catch {
      syncRequestFailed(sessionId);
    }
  }

  /** A moved draft makes the old session's result useless: sync again for a fresh one. */
  async function retrySync(id: string) {
    if (!syncSessionId.value) return;
    if (syncStatus.value?.error === SYNC_DRAFT_MOVED) {
      closeSync();
      await syncAgain(id);
      return;
    }
    await startSync(id, syncSessionId.value);
  }

  function closeSync() {
    syncSessionId.value = null;
    syncStatus.value = null;
  }

  /** Leave the agent and pick each file by hand in the conflict view. */
  async function openAdvanced(id: string) {
    const sessionId = syncSessionId.value;
    if (!sessionId) return;
    await openConflict(id, sessionId);
    closeSync();
  }

  async function onAutoResolve(id: string, event: AutoResolveStatus) {
    if (event.sessionId !== syncSessionId.value) return;
    const { sessionId, state, stage, questions, summary, error } = event;
    syncStatus.value = { sessionId, state, stage, questions, summary, error };
    if (state !== "done") return;
    await refreshTreeAndPreview();
    await Promise.all([
      load(id),
      list(id).catch(() => undefined),
      daemon.api.listConflicts(id).then(
        (sessions) => (conflictSessions.value = sessions),
        () => undefined,
      ),
    ]);
  }

  /** Re-read the active change (the agent may have started or switched one); refresh every
   *  view when it moved. */
  async function followActive(id: string) {
    const before = active.value?.branch;
    await load(id);
    if (projectId.value !== id || active.value?.branch === before) return;
    await Promise.all([refreshTreeAndPreview(active.value?.branch ?? "main"), list(id).catch(() => undefined)]);
  }

  /** Get latest: pull `main` and merge it into every open change, then refresh every view. */
  async function getLatest(id: string): Promise<ChangesUpdate> {
    updating.value = true;
    error.value = null;
    try {
      const result = await daemon.api.updateChanges(id);
      await applyUpdate(id, result);
      return result;
    } catch (e) {
      error.value = errorMessage(e);
      throw e;
    } finally {
      updating.value = false;
    }
  }

  async function loadConflicts(id: string) {
    conflictSessions.value = await daemon.api.listConflicts(id);
    const sessions = conflictSessions.value;
    const selected = sessions.find((s) => s.id === selectedConflictId.value);
    if (selected) {
      await openConflict(id, selected.id);
      return;
    }
    selectedConflictId.value = null;
    currentConflict.value = null;
    candidate.value = null;
    if (!sessions.length) return;
    const sessionId = sessions.find((s) => s.id === syncSessionId.value)?.id ?? sessions[0]!.id;
    syncSessionId.value = sessionId;
    try {
      const current = await daemon.api.autoResolveStatus(id, sessionId);
      if (syncSessionId.value === sessionId) syncStatus.value = current;
    } catch {
      if (syncSessionId.value === sessionId) syncFailed(sessionId, SYNC_REQUEST_FAILED);
    }
  }

  async function openConflict(id: string, sessionId: string) {
    conflictLoading.value = true;
    conflictError.value = null;
    try {
      const session = await daemon.api.getConflict(id, sessionId);
      projectId.value = id;
      selectedConflictId.value = sessionId;
      currentConflict.value = session;
      candidate.value = session.candidate ?? null;
    } catch (e) {
      conflictError.value = errorMessage(e);
      throw e;
    } finally {
      conflictLoading.value = false;
    }
  }

  async function chooseConflict(id: string, path: string, choice: ConflictChoice) {
    const session = currentConflict.value;
    if (!session) return;
    conflictError.value = null;
    try {
      candidate.value = await daemon.api.chooseConflict(id, session.id, path, choice);
      currentConflict.value = { ...session, choices: { ...session.choices, [path]: choice } };
    } catch (e) {
      conflictError.value = errorMessage(e);
      throw e;
    }
  }

  async function finishConflict(id: string) {
    const sessionId = selectedConflictId.value;
    if (!sessionId || !candidate.value?.complete) return;
    conflictError.value = null;
    try {
      await daemon.api.finishConflict(id, sessionId);
      selectedConflictId.value = null;
      currentConflict.value = null;
      candidate.value = null;
      await refreshTreeAndPreview();
      await Promise.all([load(id), list(id).catch(() => undefined)]);
      await loadConflicts(id);
    } catch (e) {
      conflictError.value = errorMessage(e);
      throw e;
    }
  }

  async function abandonConflict(id: string) {
    const sessionId = selectedConflictId.value;
    if (!sessionId) return;
    conflictError.value = null;
    try {
      await daemon.api.abandonConflict(id, sessionId);
      selectedConflictId.value = null;
      currentConflict.value = null;
      candidate.value = null;
      await loadConflicts(id);
    } catch (e) {
      conflictError.value = errorMessage(e);
      throw e;
    }
  }

  /**
   * Clean up a merged change: back on updated `main`, the change gone from the list.
   * `mainUpdated` is false when GitHub couldn't be reached to update `main`.
   */
  async function cleanup(id: string, branch: string): Promise<{ mainUpdated: boolean }> {
    // The change's poll must not fire mid-cleanup.
    stopStatusPolling();
    cleaningUp.value = true;
    error.value = null;
    try {
      const result = await daemon.api.cleanupChange(id, branch);
      projectId.value = id;
      items.value = items.value.filter((c) => c.branch !== branch);
      if (active.value?.branch === branch) {
        stopStatusPolling();
        active.value = null;
        prUrl.value = null;
        status.value = null;
      }
      await refreshTreeAndPreview("main");
      return { mainUpdated: result.mainUpdated !== false };
    } catch (e) {
      error.value = errorMessage(e);
      // Still on the change: keep its status fresh.
      void refreshStatus();
      throw e;
    } finally {
      cleaningUp.value = false;
    }
  }

  async function share(id: string, branch: string): Promise<string> {
    sharing.value = true;
    error.value = null;
    needsSignIn.value = false;
    protectionHint.value = null;
    try {
      const result = await daemon.api.shareChange(id, branch);
      projectId.value = id;
      prUrl.value = result.prUrl;
      protectionHint.value = result.protectionHint ?? null;
      if (active.value?.branch === branch) {
        active.value = { ...active.value, title: result.title, prUrl: result.prUrl };
        await refreshStatus();
      }
      return result.prUrl;
    } catch (e) {
      if (e instanceof ApiError && e.needsGithubSignIn) {
        needsSignIn.value = true;
        error.value = null;
      } else {
        error.value = errorMessage(e);
      }
      throw e;
    } finally {
      sharing.value = false;
    }
  }

  /** Hide the "protect main" hint for this project from now on (persisted by the daemon). */
  async function dismissProtectionHint(id: string) {
    await daemon.api.dismissProtectionHint(id);
    protectionHint.value = null;
  }

  /**
   * Mark the change's draft PR ready for review and show the status it returns. No immediate
   * re-read: GitHub's REST view can lag the GraphQL write and still say draft. Polling resumes
   * after the usual interval.
   */
  async function markReady(id: string, branch: string): Promise<void> {
    markingReady.value = true;
    needsSignIn.value = false;
    try {
      const pr = await daemon.api.markChangeReady(id, branch);
      if (projectId.value === id && active.value?.branch === branch) {
        stopStatusPolling();
        status.value = pr;
        if (pr) {
          prUrl.value = pr.url;
          pollTimer = setTimeout(() => void refreshStatus(), STATUS_POLL_MS);
        }
      }
    } catch (e) {
      if (e instanceof ApiError && e.needsGithubSignIn) {
        needsSignIn.value = true;
      }
      throw e;
    } finally {
      markingReady.value = false;
    }
  }

  return {
    projectId,
    active,
    items,
    prUrl,
    protectionHint,
    creating,
    sharing,
    markingReady,
    switching,
    updating,
    cleaningUp,
    listing,
    loading,
    error,
    needsSignIn,
    status,
    conflictSessions,
    selectedConflictId,
    currentConflict,
    candidate,
    conflictLoading,
    conflictError,
    syncSessionId,
    syncStatus,
    load,
    list,
    create,
    switchTo,
    getLatest,
    applyUpdate,
    followActive,
    loadConflicts,
    openConflict,
    chooseConflict,
    finishConflict,
    abandonConflict,
    startSync,
    answerSync,
    stopSync,
    retrySync,
    closeSync,
    openAdvanced,
    onAutoResolve,
    share,
    cleanup,
    markReady,
    refreshStatus,
    stopStatusPolling,
    dismissProtectionHint,
  };
});
