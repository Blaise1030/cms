import { defineStore } from "pinia";
import { ref } from "vue";
import { ApiError } from "@/core/api";
import { useDaemon } from "@/core/daemon";
import type { Change } from "@/core/types";
import { errorMessage } from "@/lib/errors";

export const useChangesStore = defineStore("changes", () => {
  const daemon = useDaemon();

  const projectId = ref<string | null>(null);
  const active = ref<Change | null>(null);
  const creating = ref(false);
  const loading = ref(false);
  const error = ref<string | null>(null);

  async function load(id: string) {
    projectId.value = id;
    loading.value = true;
    error.value = null;
    try {
      active.value = await daemon.api.activeChange(id);
    } catch (e) {
      if (e instanceof ApiError && e.code === "ENOENT") {
        active.value = null;
      } else {
        error.value = errorMessage(e);
      }
    } finally {
      loading.value = false;
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
      return change;
    } catch (e) {
      error.value = errorMessage(e);
      throw e;
    } finally {
      creating.value = false;
    }
  }

  return {
    projectId,
    active,
    creating,
    loading,
    error,
    load,
    create,
  };
});
