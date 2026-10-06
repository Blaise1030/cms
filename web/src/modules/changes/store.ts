import { defineStore } from "pinia";
import { ref } from "vue";
import { ApiError } from "@/core/api";
import { useDaemon } from "@/core/daemon";
import type { Change } from "@/core/types";
import { errorMessage } from "@/lib/errors";
import { useFilesStore } from "@/modules/files/store";
import { usePreviewStore } from "@/modules/preview/store";

export const useChangesStore = defineStore("changes", () => {
  const daemon = useDaemon();

  const projectId = ref<string | null>(null);
  const active = ref<Change | null>(null);
  const prUrl = ref<string | null>(null);
  const creating = ref(false);
  const sharing = ref(false);
  const loading = ref(false);
  const error = ref<string | null>(null);
  /** Share failed with expired/missing auth — show a sign-in prompt. */
  const needsSignIn = ref(false);

  async function load(id: string) {
    // Invalidate immediately so a previous project's title cannot linger.
    if (projectId.value !== id) {
      active.value = null;
      prUrl.value = null;
      error.value = null;
      needsSignIn.value = false;
    }
    projectId.value = id;
    loading.value = true;
    error.value = null;
    try {
      const change = await daemon.api.activeChange(id);
      if (projectId.value !== id) return; // stale response
      active.value = change;
      if (change.prUrl) prUrl.value = change.prUrl;
    } catch (e) {
      if (projectId.value !== id) return; // stale response
      active.value = null;
      prUrl.value = null;
      if (e instanceof ApiError && e.code === "ENOENT") {
        error.value = null;
      } else {
        error.value = errorMessage(e);
      }
    } finally {
      if (projectId.value === id) loading.value = false;
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
      // Checkout may change the tree (newer origin/main) — refresh UI.
      await useFilesStore().refresh();
      usePreviewStore().reload();
      return change;
    } catch (e) {
      error.value = errorMessage(e);
      throw e;
    } finally {
      creating.value = false;
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

  return {
    projectId,
    active,
    prUrl,
    creating,
    sharing,
    loading,
    error,
    needsSignIn,
    load,
    create,
    share,
  };
});
