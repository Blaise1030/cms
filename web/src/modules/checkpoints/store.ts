import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { Checkpoint } from "@/core/types";
import { errorMessage } from "@/lib/errors";
import { parseUnifiedDiff, type ParsedDiff } from "./diff";

export const useCheckpointsStore = defineStore("checkpoints", () => {
  const daemon = useDaemon();

  /** Newest first. */
  const byProject = ref<Record<string, Checkpoint[]>>({});
  const loading = ref(false);
  const error = ref<string | null>(null);
  const selectedSha = ref<string | null>(null);
  const diff = ref<ParsedDiff | null>(null);
  const diffLoading = ref(false);
  const diffError = ref<string | null>(null);
  const restoring = ref(false);
  let diffRequest = 0;

  function newestFirst(list: Checkpoint[]): Checkpoint[] {
    return [...list].sort((a, b) => b.ts - a.ts);
  }

  async function load(projectId: string) {
    loading.value = true;
    error.value = null;
    try {
      byProject.value[projectId] = newestFirst(await daemon.api.listCheckpoints(projectId));
    } catch (e) {
      error.value = errorMessage(e);
    } finally {
      loading.value = false;
    }
  }

  async function select(projectId: string, sha: string | null) {
    selectedSha.value = sha;
    diff.value = null;
    diffError.value = null;
    if (!sha) return;
    const request = ++diffRequest;
    diffLoading.value = true;
    try {
      const parsed = parseUnifiedDiff(await daemon.api.checkpointDiff(projectId, sha));
      if (request === diffRequest) diff.value = parsed;
    } catch (e) {
      if (request === diffRequest) diffError.value = errorMessage(e);
    } finally {
      if (request === diffRequest) diffLoading.value = false;
    }
  }

  /** Restore a checkpoint (`git reset --hard`), then refresh the timeline. */
  async function restore(projectId: string, sha: string) {
    restoring.value = true;
    try {
      await daemon.api.restoreCheckpoint(projectId, sha);
      await select(projectId, null);
      await load(projectId);
    } finally {
      restoring.value = false;
    }
  }

  const offs = [
    daemon.on("checkpoint.created", (e) => {
      const list = byProject.value[e.projectId];
      if (!list) return; // never opened; loaded fresh on first view
      if (list.some((c) => c.sha === e.sha)) return;
      byProject.value[e.projectId] = [
        { sha: e.sha, summary: e.summary, ts: Math.floor(Date.now() / 1000) },
        ...list,
      ];
    }),
    daemon.onReconnected(() => {
      for (const id of Object.keys(byProject.value)) void load(id);
    }),
  ];
  onScopeDispose(() => offs.forEach((off) => off()));

  return {
    byProject,
    loading,
    error,
    selectedSha,
    diff,
    diffLoading,
    diffError,
    restoring,
    load,
    select,
    restore,
  };
});
