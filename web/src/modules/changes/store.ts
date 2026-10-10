import { defineStore } from "pinia";
import { ref } from "vue";
import { ApiError } from "@/core/api";
import { useDaemon } from "@/core/daemon";
import type { Change, PrStatus } from "@/core/types";
import { errorMessage } from "@/lib/errors";
import { useFilesStore } from "@/modules/files/store";
import { usePreviewStore } from "@/modules/preview/store";

/** How often the active change's PR status is re-read while it has a PR. */
export const STATUS_POLL_MS = 20_000;

export const useChangesStore = defineStore("changes", () => {
  const daemon = useDaemon();

  const projectId = ref<string | null>(null);
  const active = ref<Change | null>(null);
  const items = ref<Change[]>([]);
  const prUrl = ref<string | null>(null);
  const creating = ref(false);
  const sharing = ref(false);
  const markingReady = ref(false);
  const switching = ref(false);
  const cleaningUp = ref(false);
  const listing = ref(false);
  const loading = ref(false);
  const error = ref<string | null>(null);
  /** Share failed with expired/missing auth — show a sign-in prompt. */
  const needsSignIn = ref(false);
  /** PR state / checks / preview URL of the active change; null without a PR. */
  const status = ref<PrStatus | null>(null);
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

  async function refreshTreeAndPreview() {
    await useFilesStore().refresh();
    usePreviewStore().reload();
  }

  async function load(id: string) {
    if (projectId.value !== id) {
      stopStatusPolling();
      active.value = null;
      prUrl.value = null;
      status.value = null;
      items.value = [];
      error.value = null;
      needsSignIn.value = false;
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
      await refreshTreeAndPreview();
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
      await refreshTreeAndPreview();
      await refreshStatus();
      return change;
    } catch (e) {
      error.value = errorMessage(e);
      throw e;
    } finally {
      switching.value = false;
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
      await refreshTreeAndPreview();
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
    try {
      const result = await daemon.api.shareChange(id, branch);
      projectId.value = id;
      prUrl.value = result.prUrl;
      if (active.value?.branch === branch) {
        active.value = { ...active.value, title: result.title, prUrl: result.prUrl };
        await refreshStatus();
      }
      return result.prUrl;
    } catch (e) {
      if (
        e instanceof ApiError &&
        (e.isUnauthorized || e.code === "UNAUTH" || e.status === 401)
      ) {
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
      if (
        e instanceof ApiError &&
        (e.isUnauthorized || e.code === "UNAUTH" || e.status === 401)
      ) {
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
    creating,
    sharing,
    markingReady,
    switching,
    cleaningUp,
    listing,
    loading,
    error,
    needsSignIn,
    status,
    load,
    list,
    create,
    switchTo,
    share,
    cleanup,
    markReady,
    refreshStatus,
    stopStatusPolling,
  };
});
