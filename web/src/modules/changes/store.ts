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
  const items = ref<Change[]>([]);
  const creating = ref(false);
  const switching = ref(false);
  const listing = ref(false);
  const loading = ref(false);
  const error = ref<string | null>(null);

  async function refreshTreeAndPreview() {
    await useFilesStore().refresh();
    usePreviewStore().reload();
  }

  async function load(id: string) {
    // Invalidate immediately so a previous project's title cannot linger.
    if (projectId.value !== id) {
      active.value = null;
      items.value = [];
      error.value = null;
    }
    projectId.value = id;
    loading.value = true;
    error.value = null;
    try {
      const change = await daemon.api.activeChange(id);
      if (projectId.value !== id) return; // stale response
      active.value = change;
    } catch (e) {
      if (projectId.value !== id) return; // stale response
      active.value = null;
      if (e instanceof ApiError && e.code === "ENOENT") {
        error.value = null;
      } else {
        error.value = errorMessage(e);
      }
    } finally {
      if (projectId.value === id) loading.value = false;
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
    try {
      const change = await daemon.api.createChange(id, trimmed);
      projectId.value = id;
      active.value = change;
      // Checkout may change the tree (newer origin/main) — refresh UI.
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
    switching.value = true;
    error.value = null;
    try {
      const change = await daemon.api.switchChange(id, branch);
      projectId.value = id;
      active.value = change;
      await refreshTreeAndPreview();
      return change;
    } catch (e) {
      error.value = errorMessage(e);
      throw e;
    } finally {
      switching.value = false;
    }
  }

  return {
    projectId,
    active,
    items,
    creating,
    switching,
    listing,
    loading,
    error,
    load,
    list,
    create,
    switchTo,
  };
});
