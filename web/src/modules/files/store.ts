import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { FileContent } from "@/core/types";
import { errorMessage } from "@workbench/utils";

const REFRESH_DELAY_MS = 250;

/** Skip heavy / generated dirs when walking for the Pierre tree. */
const SKIP_DIRS = new Set([
  "node_modules",
  "dist",
  ".astro",
  ".next",
  ".output",
  "coverage",
  "target",
  ".vite",
]);

export const useFilesStore = defineStore("files", () => {
  const daemon = useDaemon();

  const projectId = ref<string | null>(null);
  /** Flat file paths for `@pierre/trees` (directories are inferred from segments). */
  const paths = ref<string[]>([]);
  const selectedPath = ref<string | null>(null);
  const file = ref<FileContent | null>(null);
  const fileLoading = ref(false);
  const fileError = ref<string | null>(null);
  const treeError = ref<string | null>(null);
  const treeLoading = ref(false);
  let refreshTimer: ReturnType<typeof setTimeout> | undefined;

  async function collectFilePaths(id: string, dir: string): Promise<string[]> {
    const tree = await daemon.api.tree(id, dir);
    const out: string[] = [];
    for (const entry of tree.entries) {
      if (entry.kind === "file") {
        out.push(entry.path);
        continue;
      }
      if (SKIP_DIRS.has(entry.name)) continue;
      out.push(...(await collectFilePaths(id, entry.path)));
    }
    return out;
  }

  async function loadTree(id: string) {
    treeLoading.value = true;
    try {
      paths.value = await collectFilePaths(id, "");
      treeError.value = null;
    } catch (e) {
      if (projectId.value === id) {
        paths.value = [];
        treeError.value = errorMessage(e);
      }
    } finally {
      if (projectId.value === id) treeLoading.value = false;
    }
  }

  async function loadFile(id: string, path: string) {
    fileLoading.value = true;
    fileError.value = null;
    try {
      const content = await daemon.api.file(id, path);
      if (projectId.value === id && selectedPath.value === path) file.value = content;
    } catch (e) {
      if (projectId.value === id && selectedPath.value === path) {
        file.value = null;
        fileError.value = errorMessage(e);
      }
    } finally {
      if (projectId.value === id && selectedPath.value === path) fileLoading.value = false;
    }
  }

  /** Point the store at a project (resets state when it changed) and load the file list. */
  async function open(id: string) {
    if (projectId.value !== id) {
      clearTimeout(refreshTimer);
      projectId.value = id;
      paths.value = [];
      selectedPath.value = null;
      file.value = null;
      fileError.value = null;
      treeError.value = null;
    }
    await loadTree(id);
  }

  async function select(path: string) {
    const id = projectId.value;
    if (!id) return;
    selectedPath.value = path;
    file.value = null;
    await loadFile(id, path);
  }

  async function refresh() {
    const id = projectId.value;
    if (!id) return;
    await loadTree(id);
    if (selectedPath.value) await loadFile(id, selectedPath.value);
  }

  const offs = [
    daemon.on("fs.change", (e) => {
      if (e.projectId !== projectId.value) return;
      clearTimeout(refreshTimer);
      refreshTimer = setTimeout(() => void refresh(), REFRESH_DELAY_MS);
    }),
    daemon.onReconnected(() => void refresh()),
    daemon.on("events.lagged", () => void refresh()),
  ];
  onScopeDispose(() => {
    clearTimeout(refreshTimer);
    offs.forEach((off) => off());
  });

  return {
    projectId,
    paths,
    selectedPath,
    file,
    fileLoading,
    fileError,
    treeError,
    treeLoading,
    open,
    select,
    refresh,
  };
});
