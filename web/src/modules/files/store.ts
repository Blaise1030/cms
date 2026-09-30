import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { FileContent, TreeEntry } from "@/core/types";
import { errorMessage } from "@/lib/errors";

const REFRESH_DELAY_MS = 250;

export const useFilesStore = defineStore("files", () => {
  const daemon = useDaemon();

  const projectId = ref<string | null>(null);
  /** Directory listings by path (`""` is the project root). */
  const dirs = ref<Record<string, TreeEntry[]>>({});
  const expanded = ref<Record<string, boolean>>({});
  const selectedPath = ref<string | null>(null);
  const file = ref<FileContent | null>(null);
  const fileLoading = ref(false);
  const fileError = ref<string | null>(null);
  const treeError = ref<string | null>(null);
  let refreshTimer: ReturnType<typeof setTimeout> | undefined;

  async function loadDir(id: string, path: string) {
    try {
      const tree = await daemon.api.tree(id, path);
      if (projectId.value !== id) return; // switched projects meanwhile
      dirs.value[path] = [...tree.entries].sort(
        (a, b) => (a.kind === b.kind ? a.name.localeCompare(b.name) : a.kind === "dir" ? -1 : 1),
      );
      treeError.value = null;
    } catch (e) {
      if (projectId.value === id) treeError.value = errorMessage(e);
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

  /** Point the store at a project (resets state when it changed) and load the root. */
  async function open(id: string) {
    if (projectId.value !== id) {
      clearTimeout(refreshTimer);
      projectId.value = id;
      dirs.value = {};
      expanded.value = {};
      selectedPath.value = null;
      file.value = null;
      fileError.value = null;
      treeError.value = null;
    }
    await loadDir(id, "");
  }

  async function toggle(path: string) {
    const id = projectId.value;
    if (!id) return;
    const next = !expanded.value[path];
    expanded.value[path] = next;
    if (next && !dirs.value[path]) await loadDir(id, path);
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
    const paths = ["", ...Object.keys(expanded.value).filter((p) => expanded.value[p])];
    await Promise.all(paths.map((p) => loadDir(id, p)));
    if (selectedPath.value) await loadFile(id, selectedPath.value);
  }

  const offs = [
    daemon.on("fs.change", (e) => {
      if (e.projectId !== projectId.value) return;
      clearTimeout(refreshTimer);
      refreshTimer = setTimeout(() => void refresh(), REFRESH_DELAY_MS);
    }),
    daemon.onReconnected(() => void refresh()),
  ];
  onScopeDispose(() => {
    clearTimeout(refreshTimer);
    offs.forEach((off) => off());
  });

  return {
    projectId,
    dirs,
    expanded,
    selectedPath,
    file,
    fileLoading,
    fileError,
    treeError,
    open,
    toggle,
    select,
    refresh,
  };
});
