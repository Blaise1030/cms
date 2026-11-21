import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { Checkpoint } from "@/core/types";
import { errorMessage } from "@orso/utils";
import type { ParsedDiff } from "./diff";

export const useCheckpointsStore = defineStore("checkpoints", () => {
  const daemon = useDaemon();

  /** Newest first. */
  const byProject = ref<Record<string, Checkpoint[]>>({});
  /** The folder each timeline covers (`.` = whole repo) and the commit its project is at. */
  const rootByProject = ref<Record<string, string>>({});
  const headByProject = ref<Record<string, string | null>>({});
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
      const log = await daemon.api.listCheckpoints(projectId);
      byProject.value[projectId] = newestFirst(log.checkpoints);
      rootByProject.value[projectId] = log.root;
      headByProject.value[projectId] = log.head;
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
      // Lazy: the parser lives in `@pierre/diffs`, kept out of the app shell with the renderer.
      const [{ parseUnifiedDiff }, text] = await Promise.all([
        import("./diff"),
        daemon.api.checkpointDiff(projectId, sha),
      ]);
      const parsed = parseUnifiedDiff(text);
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

  /** Missed `checkpoint.created` events: reload every timeline we have shown. */
  function resync() {
    for (const id of Object.keys(byProject.value)) void load(id);
  }

  const offs = [
    daemon.on("checkpoint.created", (e) => {
      const list = byProject.value[e.projectId];
      if (!list) return; // never opened; loaded fresh on first view
      // A site-scoped timeline lists only commits touching the site folder; the event does
      // not say which files changed, so ask the daemon again.
      if ((rootByProject.value[e.projectId] ?? ".") !== ".") return void load(e.projectId);
      headByProject.value[e.projectId] = e.sha;
      if (list.some((c) => c.sha === e.sha)) return;
      byProject.value[e.projectId] = [
        { sha: e.sha, summary: e.summary, author: e.author, ts: e.ts, avatarUrl: e.avatarUrl },
        ...list,
      ];
      // Whether it went out depends on the branch; the daemon's timeline says.
      if (list.some((c) => c.published !== undefined)) void load(e.projectId);
    }),
    daemon.onReconnected(resync),
    daemon.on("events.lagged", resync),
  ];
  onScopeDispose(() => offs.forEach((off) => off()));

  return {
    byProject,
    rootByProject,
    headByProject,
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
