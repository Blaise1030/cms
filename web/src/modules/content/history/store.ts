import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { ApiError } from "@/core/api";
import { useDaemon } from "@/core/daemon";
import type { ContentVersion, FileVersion } from "@/core/types";
import { errorMessage } from "@/lib/errors";

export const useContentHistoryStore = defineStore("contentHistory", () => {
  const daemon = useDaemon();

  /** `collection/id` of the open entry. */
  const key = ref<string | null>(null);
  /** Newest first. */
  const versions = ref<FileVersion[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const selectedSha = ref<string | null>(null);
  const version = ref<ContentVersion | null>(null);
  const versionLoading = ref(false);
  let entry: { project: string; collection: string; id: string } | null = null;
  let loadRequest = 0;
  let versionRequest = 0;

  async function reload() {
    if (!entry) return;
    const { project, collection, id } = entry;
    const request = ++loadRequest;
    loading.value = true;
    error.value = null;
    try {
      const list = await daemon.api.contentHistory(project, collection, id);
      if (request !== loadRequest) return;
      versions.value = list;
      if (selectedSha.value && !list.some((v) => v.sha === selectedSha.value)) void select(null);
    } catch (e) {
      if (request === loadRequest) error.value = errorMessage(e);
    } finally {
      if (request === loadRequest) loading.value = false;
    }
  }

  async function select(sha: string | null) {
    selectedSha.value = sha;
    version.value = null;
    const request = ++versionRequest;
    if (!sha || !entry) {
      versionLoading.value = false;
      return;
    }
    const { project, collection, id } = entry;
    versionLoading.value = true;
    try {
      const v = await daemon.api.contentVersion(project, collection, id, sha);
      if (request === versionRequest) version.value = v;
    } catch (e) {
      if (request !== versionRequest) return;
      // HEAD moved and the commit is no longer in this entry's history: refresh the list instead.
      if (e instanceof ApiError && e.status === 404) {
        selectedSha.value = null;
        await reload();
      } else {
        error.value = errorMessage(e);
      }
    } finally {
      if (request === versionRequest) versionLoading.value = false;
    }
  }

  async function open(project: string, collection: string, id: string) {
    entry = { project, collection, id };
    key.value = `${collection}/${id}`;
    versions.value = [];
    await select(null);
    await reload();
  }

  function close() {
    entry = null;
    key.value = null;
    versions.value = [];
    error.value = null;
    loadRequest++;
    loading.value = false;
    void select(null);
  }

  const offs = [
    daemon.on("checkpoint.created", (e) => {
      if (key.value && entry?.project === e.projectId) void reload();
    }),
    daemon.onReconnected(() => void reload()),
  ];
  onScopeDispose(() => offs.forEach((off) => off()));

  return {
    key,
    versions,
    loading,
    error,
    selectedSha,
    version,
    versionLoading,
    open,
    reload,
    select,
    close,
  };
});
