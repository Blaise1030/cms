import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { FileContent } from "@/core/types";
import { errorMessage } from "@orso/utils";

const REFRESH_DELAY_MS = 250;
/** How many `/api/tree` requests the walk keeps in flight. */
const TREE_CONCURRENCY = 8;

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

/** Run at most `n` tasks at once; the rest wait their turn. */
function limiter(n: number) {
  let active = 0;
  const waiting: (() => void)[] = [];
  return async <T>(task: () => Promise<T>): Promise<T> => {
    // A finished task hands its slot straight to the next waiter, so `active` never exceeds `n`.
    if (active >= n) await new Promise<void>((resolve) => waiting.push(resolve));
    else active++;
    try {
      return await task();
    } finally {
      const next = waiting.shift();
      if (next) next();
      else active--;
    }
  };
}

/** The configured site folder (`orso.json` `root`) as a tree path: `""` for the repo root. */
async function siteRoot(daemon: ReturnType<typeof useDaemon>, id: string): Promise<string> {
  try {
    const root = (await daemon.api.settings(id)).siteRoot;
    return !root || root === "." ? "" : root.replace(/\/+$/, "");
  } catch {
    return ""; // unreadable settings: the daemon falls back to the repo root too
  }
}

export const useFilesStore = defineStore("files", () => {
  const daemon = useDaemon();

  const projectId = ref<string | null>(null);
  /** Flat file paths for `@pierre/trees` (directories are inferred from segments). */
  const paths = ref<string[]>([]);
  /** The site folder the tree is scoped to (`""` = the whole repo); `paths` stay repo-relative. */
  const root = ref("");
  const selectedPath = ref<string | null>(null);
  const file = ref<FileContent | null>(null);
  const fileLoading = ref(false);
  const fileError = ref<string | null>(null);
  const treeError = ref<string | null>(null);
  const treeLoading = ref(false);
  let refreshTimer: ReturnType<typeof setTimeout> | undefined;
  let treeRequest = 0;

  /** Every file under `dir`, in tree order; sibling folders are listed in parallel (bounded). */
  function collectFilePaths(id: string, dir: string): Promise<string[]> {
    const limit = limiter(TREE_CONCURRENCY);
    const walk = async (at: string): Promise<string[]> => {
      const tree = await limit(() => daemon.api.tree(id, at));
      const nested = await Promise.all(
        tree.entries.map((entry) => {
          if (entry.kind === "file") return [entry.path];
          if (SKIP_DIRS.has(entry.name)) return [];
          return walk(entry.path);
        }),
      );
      return nested.flat();
    };
    return walk(dir);
  }

  /** Re-read the site folder from settings, then list the files under it. */
  async function loadTree(id: string) {
    const request = ++treeRequest;
    const current = () => request === treeRequest && projectId.value === id;
    treeLoading.value = true;
    try {
      const dir = await siteRoot(daemon, id);
      const found = await collectFilePaths(id, dir);
      if (!current()) return;
      root.value = dir;
      paths.value = found;
      treeError.value = null;
    } catch (e) {
      if (current()) {
        paths.value = [];
        treeError.value = errorMessage(e);
      }
    } finally {
      if (current()) treeLoading.value = false;
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
      root.value = "";
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
    root,
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
