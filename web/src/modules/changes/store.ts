import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { toast } from "vue-sonner";
import { ApiError } from "@/core/api";
import { useDaemon } from "@/core/daemon";
import type { Change, ChangesUpdate, ConflictCandidate, ConflictChoice, ConflictSession, PrStatus, ProtectionHint, SyncChangedEvent } from "@/core/types";
import { errorMessage } from "@orso/utils";
import { useFilesStore } from "@/modules/files/store";
import { usePreviewStore } from "@/modules/preview/store";
import { useContentStore } from "@/modules/content/store";
import { useDesignStore } from "@/modules/design/store";
import { useSettingsStore } from "@/modules/settings/store";
import { useAgentProvidersStore } from "@/modules/chat/providers";
import { useChatStore } from "@/modules/chat/store";

const SYNC_UNAVAILABLE = "Your assistant isn't available.";
const SYNC_START_FAILED = "Couldn't start combining. Please try again.";
const SYNC_REQUEST_FAILED = "Something went wrong. Please try again.";

/** A switch held until the user confirms, because a chat turn is running. */
interface PendingSwitch {
  projectId: string;
  branch: string;
  fromTitle: string;
  toTitle: string;
}

/** A share/publish that hit a colliding remote copy, retried once combining succeeds. */
interface PendingPush {
  kind: "share" | "publish";
  projectId: string;
  branch: string;
}

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
  const publishing = ref(false);
  const switching = ref(false);
  const updating = ref(false);
  const cleaningUp = ref(false);
  const listing = ref(false);
  const loading = ref(false);
  const error = ref<string | null>(null);
  /** A switch paused for confirmation because a chat turn is running. */
  const pendingSwitch = ref<PendingSwitch | null>(null);
  /** Share failed with expired/missing auth — show a sign-in prompt. */
  const needsSignIn = ref(false);
  /** PR state / checks / preview URL of the active change; null without a PR. */
  const status = ref<PrStatus | null>(null);
  /** Production site URL while on `main`; null on a change or when unknown. */
  const liveUrl = ref<string | null>(null);
  /** Whether Get latest would change anything; null while unknown (the button stays enabled). */
  const syncNeeded = ref<boolean | null>(null);
  const conflictSessions = ref<ConflictSession[]>([]);
  const selectedConflictId = ref<string | null>(null);
  const currentConflict = ref<ConflictSession | null>(null);
  const candidate = ref<ConflictCandidate | null>(null);
  const conflictLoading = ref(false);
  const conflictError = ref<string | null>(null);
  /** The conflict session whose sync chat is open; null when there is none. */
  const syncSessionId = ref<string | null>(null);
  /** A plain sentence about why starting or stopping a combine failed. */
  const syncError = ref<string | null>(null);
  /** The share/publish to retry once the writer's assistant has combined a push collision. */
  const pendingPush = ref<PendingPush | null>(null);
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
    await refreshSync();
    if (run !== statusRun) return;
    if (status.value) {
      pollTimer = setTimeout(() => void refreshStatus(), STATUS_POLL_MS);
    }
  }

  /**
   * Re-read the production site URL while on `main`. Failures and change branches clear it
   * so the header link stays hidden rather than pointing at a stale preview.
   */
  async function refreshLiveUrl() {
    const id = projectId.value;
    if (!id || active.value) {
      liveUrl.value = null;
      return;
    }
    try {
      liveUrl.value = await daemon.api.liveSiteUrl(id);
    } catch {
      liveUrl.value = null;
    }
  }

  /**
   * Re-read whether Get latest would change anything. A failure leaves it unknown, so the
   * button stays enabled rather than being wrongly disabled.
   */
  async function refreshSync() {
    const id = projectId.value;
    if (!id) {
      syncNeeded.value = null;
      return;
    }
    try {
      const needed = await daemon.api.changesSyncNeeded(id);
      if (projectId.value === id) syncNeeded.value = needed;
    } catch {
      if (projectId.value === id) syncNeeded.value = null;
    }
  }

  /** Refresh every view after the working tree moved; `branch` when the project switched to it. */
  async function refreshTreeAndPreview(branch?: string) {
    // One failing refresh (content cannot be served until the preview is up) must not
    // block the others or the preview reload: each store surfaces its own error.
    await Promise.allSettled([
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
      liveUrl.value = null;
      syncNeeded.value = null;
      protectionHint.value = null;
      items.value = [];
      error.value = null;
      needsSignIn.value = false;
      conflictSessions.value = [];
      selectedConflictId.value = null;
      currentConflict.value = null;
      candidate.value = null;
      syncSessionId.value = null;
      syncError.value = null;
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
    if (projectId.value !== id) return;
    if (active.value) {
      liveUrl.value = null;
      await refreshStatus();
    } else {
      await Promise.all([refreshSync(), refreshLiveUrl()]);
    }
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
      liveUrl.value = null;
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

  async function switchTo(id: string, branch: string): Promise<Change | null> {
    // A chat turn is running for this project: hold the switch until the user confirms,
    // so the agent is never left mid-thought on the wrong draft.
    if (useChatStore().runningChatIds(id).length) {
      pendingSwitch.value = {
        projectId: id,
        branch,
        fromTitle: active.value?.title ?? "the live site",
        toTitle: items.value.find((c) => c.branch === branch)?.title ?? branch,
      };
      return null;
    }
    return performSwitch(id, branch);
  }

  async function performSwitch(id: string, branch: string): Promise<Change> {
    toast.info("Opening this draft…");
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
      liveUrl.value = null;
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

  /** Continue the held switch: stop the chat run first, then switch. A failed stop aborts. */
  async function confirmSwitch(): Promise<Change> {
    const pending = pendingSwitch.value;
    if (!pending) throw new Error("Nothing to confirm");
    pendingSwitch.value = null;
    try {
      const chats = useChatStore();
      for (const runningId of chats.runningChatIds(pending.projectId)) {
        chats.setQueuePaused(pending.projectId, runningId, true);
        await daemon.api.stopChat(pending.projectId, runningId);
      }
    } catch (e) {
      error.value = errorMessage(e);
      throw e;
    }
    return performSwitch(pending.projectId, pending.branch);
  }

  function cancelSwitch() {
    pendingSwitch.value = null;
  }

  /** Refresh every view after a Get latest (ours, or one the daemon finished later). */
  async function applyUpdate(id: string, result: ChangesUpdate) {
    await refreshTreeAndPreview();
    // Branch tips (and so "last edited") moved; a failed list refresh is not an update failure.
    await Promise.all([load(id), list(id).catch(() => undefined)]);
    const first = result.conflicts.find((c) => result.conflictSessions?.[c.branch]);
    if (first) await startSyncChat(id, result.conflictSessions![first.branch]!);
  }

  /**
   * Open the chat that combines the conflict session with the latest site. The
   * chat is the whole surface now: its transcript carries progress and questions.
   */
  async function startSyncChat(id: string, sessionId: string) {
    const provider = useAgentProvidersStore().provider;
    if (!provider) {
      syncError.value = SYNC_UNAVAILABLE;
      toast.error(SYNC_UNAVAILABLE);
      return;
    }
    syncError.value = null;
    try {
      const { chatId } = await daemon.api.startSyncChat(id, sessionId, provider);
      syncSessionId.value = sessionId;
      const chats = useChatStore();
      await chats.load(id);
      chats.setActive(id, chatId);
    } catch {
      // The daemon's own copy talks about git, so only our plain sentence is shown.
      syncError.value = SYNC_START_FAILED;
      toast.error(SYNC_START_FAILED);
    }
  }

  /**
   * End a sync chat, releasing the draft for manual choices. The session and chat come
   * from the chat's own summary when the caller has them; the remembered session is a
   * fallback for callers that do not.
   */
  async function endSyncChat(
    id: string,
    sessionId = syncSessionId.value,
    chatId = useChatStore().activeChatId(id),
  ) {
    if (!sessionId || !chatId) return;
    syncError.value = null;
    // The writer cancelled the combine: a share/publish held for it must not fire later.
    pendingPush.value = null;
    try {
      await daemon.api.endSyncChat(id, sessionId, chatId);
      if (syncSessionId.value === sessionId) syncSessionId.value = null;
      await useChatStore().load(id);
    } catch (e) {
      // Already ended elsewhere: there is nothing left to stop.
      if (e instanceof ApiError && e.status === 404) {
        if (syncSessionId.value === sessionId) syncSessionId.value = null;
        return;
      }
      syncError.value = SYNC_REQUEST_FAILED;
      toast.error(SYNC_REQUEST_FAILED);
    }
  }

  /** Leave the chat and pick each file by hand in the conflict view. */
  async function pickMyself(id: string, sessionId = syncSessionId.value) {
    if (!sessionId) return;
    try {
      await openConflict(id, sessionId);
    } catch {
      // The session may be gone (a combine can finish, or another tab ended it).
      // Never reject: the click gets one plain line instead of an unhandled error.
      toast.error(SYNC_REQUEST_FAILED);
    }
  }

  /** Ask the assistant to combine this draft again, opening a fresh chat. */
  async function combineAgain(id: string, sessionId = syncSessionId.value) {
    if (!sessionId) return;
    await startSyncChat(id, sessionId);
  }

  /** Continue the share/publish that hit a collision, now that combining succeeded. */
  async function retryPendingPush(id: string) {
    const pending = pendingPush.value;
    if (!pending || pending.projectId !== id) return;
    pendingPush.value = null;
    try {
      if (pending.kind === "publish") {
        if ((await publish(id, pending.branch)) === "published") {
          toast.success("It's live.");
        }
      } else {
        await share(id, pending.branch);
      }
    } catch (e) {
      toast.error(errorMessage(e));
    }
  }

  /** Follow the open sync chat; when it lands, refresh everything and finish a pending push. */
  async function onSyncChanged(id: string, event: SyncChangedEvent) {
    if (event.projectId !== id || event.conflictSessionId !== syncSessionId.value) return;
    await useChatStore().load(id);
    if (event.phase !== "combined") return;
    await refreshTreeAndPreview();
    await Promise.all([
      load(id),
      list(id).catch(() => undefined),
      daemon.api.listConflicts(id).then(
        (sessions) => (conflictSessions.value = sessions),
        () => undefined,
      ),
    ]);
    await retryPendingPush(id);
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
    // Only one combine can run per project; try the remembered session first, then the rest.
    const ordered = sessions
      .filter((s) => s.id === syncSessionId.value)
      .concat(sessions.filter((s) => s.id !== syncSessionId.value));
    for (const session of ordered) {
      try {
        const status = await daemon.api.syncChatStatus(id, session.id);
        syncSessionId.value = session.id;
        const chats = useChatStore();
        await chats.load(id);
        chats.setActive(id, status.chatId);
        return;
      } catch (e) {
        if (e instanceof ApiError && e.status === 404) continue;
        return;
      }
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
   * Return to the live site (`main`). Open changes are kept; a change that is no longer
   * open (already on `main`) is cleaned up. No-op when already on the live site.
   */
  async function goLive(id: string): Promise<void> {
    const branch = active.value?.branch;
    if (!branch) return;
    const previous = items.value;
    try {
      await list(id);
    } catch {
      // Keep the last known open list when refresh fails.
      items.value = previous;
    }
    const stillOpen = items.value.some((c) => c.branch === branch);
    if (stillOpen) {
      switching.value = true;
      error.value = null;
      stopStatusPolling();
      try {
        await daemon.api.leaveChange(id);
        projectId.value = id;
        active.value = null;
        prUrl.value = null;
        status.value = null;
        protectionHint.value = null;
        await refreshTreeAndPreview("main");
        void refreshSync();
        void refreshLiveUrl();
      } catch (e) {
        error.value = errorMessage(e);
        void refreshStatus();
        throw e;
      } finally {
        switching.value = false;
      }
      return;
    }
    const { mainUpdated } = await cleanup(id, branch);
    if (!mainUpdated) {
      toast.warning(
        "Cleaned up, but main couldn't be updated from GitHub — it may be out of date",
      );
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
      void refreshSync();
      void refreshLiveUrl();
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

  async function share(id: string, branch: string): Promise<string | null> {
    sharing.value = true;
    error.value = null;
    needsSignIn.value = false;
    protectionHint.value = null;
    try {
      const result = await daemon.api.shareChange(id, branch);
      projectId.value = id;
      if (result.conflict) {
        pendingPush.value = { kind: "share", projectId: id, branch };
        await startSyncChat(id, result.conflict.sessionId);
        return null;
      }
      prUrl.value = result.prUrl;
      protectionHint.value = result.protectionHint ?? null;
      if (active.value?.branch === branch) {
        active.value = { ...active.value, title: result.title, prUrl: result.prUrl ?? undefined };
        await refreshStatus();
      }
      void refreshSync();
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

  // Toast "Updating tools…" only when the installing project is the one the writer has open,
  // and remember the install: the switch-time refresh ran before the preview could serve
  // content, so content (like the preview) must refresh once the install is done.
  let installPending = false;
  const offDepsInstalling = daemon.on("preview.deps_installing", (e) => {
    if (e.projectId !== projectId.value) return;
    installPending = true;
    toast.info("Updating tools for this draft…");
  });
  const offDevserverStatus = daemon.on("devserver.status", (e) => {
    if (e.projectId !== projectId.value || e.state !== "ready" || !installPending) return;
    installPending = false;
    void refreshTreeAndPreview(active.value?.branch ?? "main");
  });
  onScopeDispose(() => {
    offDepsInstalling();
    offDevserverStatus();
  });

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

  /**
   * Publish the change: push it, share it (opening a pull request if it has none), mark it
   * ready, merge it on GitHub, then clean up onto updated `main`. Clean up stays available when
   * the merge happened outside orso, or when this post-publish cleanup fails.
   *
   * Returns `combining` when someone else moved the change first: the combine dialog opens and
   * publishing retries automatically once it succeeds.
   */
  async function publish(id: string, branch: string): Promise<"published" | "combining"> {
    publishing.value = true;
    needsSignIn.value = false;
    try {
      const result = await daemon.api.publishChange(id, branch);
      projectId.value = id;
      if (result && "conflict" in result) {
        pendingPush.value = { kind: "publish", projectId: id, branch };
        await startSyncChat(id, result.conflict.sessionId);
        return "combining";
      }
      const { mainUpdated } = await cleanup(id, branch);
      if (!mainUpdated) {
        toast.warning(
          "Cleaned up, but main couldn't be updated from GitHub — it may be out of date",
        );
      }
      return "published";
    } catch (e) {
      if (e instanceof ApiError && e.needsGithubSignIn) {
        needsSignIn.value = true;
      }
      throw e;
    } finally {
      publishing.value = false;
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
    publishing,
    switching,
    updating,
    cleaningUp,
    listing,
    loading,
    error,
    pendingSwitch,
    needsSignIn,
    status,
    liveUrl,
    syncNeeded,
    conflictSessions,
    selectedConflictId,
    currentConflict,
    candidate,
    conflictLoading,
    conflictError,
    syncSessionId,
    syncError,
    pendingPush,
    load,
    list,
    create,
    switchTo,
    confirmSwitch,
    cancelSwitch,
    getLatest,
    applyUpdate,
    followActive,
    loadConflicts,
    openConflict,
    chooseConflict,
    finishConflict,
    abandonConflict,
    startSyncChat,
    endSyncChat,
    pickMyself,
    combineAgain,
    onSyncChanged,
    share,
    goLive,
    cleanup,
    markReady,
    publish,
    refreshStatus,
    stopStatusPolling,
    dismissProtectionHint,
  };
});
