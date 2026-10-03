import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { Project } from "@/core/types";
import { errorMessage } from "@/lib/errors";

export const useProjectsStore = defineStore("projects", () => {
  const daemon = useDaemon();

  const projects = ref<Project[]>([]);
  const loading = ref(false);
  const loaded = ref(false);
  const creating = ref(false);
  const error = ref<string | null>(null);

  async function load() {
    loading.value = true;
    error.value = null;
    try {
      projects.value = await daemon.api.listProjects();
      loaded.value = true;
    } catch (e) {
      error.value = errorMessage(e);
    } finally {
      loading.value = false;
    }
  }

  /** Create a project (the caller navigates to it). Throws on failure so the dialog can show the error. */
  async function create(name: string): Promise<Project> {
    const trimmed = name.trim();
    if (!trimmed) throw new Error("Project name is required");
    creating.value = true;
    try {
      const project = await daemon.api.createProject(trimmed);
      projects.value = [...projects.value.filter((p) => p.id !== project.id), project];
      return project;
    } finally {
      creating.value = false;
    }
  }

  /** Open a GitHub repository as a project (the daemon clones it). */
  async function cloneRepo(repo: string): Promise<Project> {
    creating.value = true;
    try {
      const project = await daemon.api.cloneProject(repo);
      projects.value = [...projects.value.filter((p) => p.id !== project.id), project];
      return project;
    } finally {
      creating.value = false;
    }
  }

  /** Delete a project (the api client sends the daemon's mandatory `?confirm=true`). */
  async function remove(id: string) {
    await daemon.api.deleteProject(id);
    projects.value = projects.value.filter((p) => p.id !== id);
  }

  async function setAutoApprove(id: string, autoApprove: boolean) {
    const updated = await daemon.api.updateProject(id, { autoApprove });
    projects.value = projects.value.map((p) => (p.id === id ? { ...p, ...updated } : p));
  }

  const offs = [
    daemon.on("devserver.status", (e) => {
      projects.value = projects.value.map((p) =>
        p.id === e.projectId
          ? { ...p, devserver: e.state, port: e.port ?? p.port }
          : p,
      );
    }),
    // Events in a gap (socket drop, or the server reporting `events.lagged`) are gone: refetch.
    daemon.onReconnected(() => void load()),
    daemon.on("events.lagged", () => void load()),
  ];
  onScopeDispose(() => offs.forEach((off) => off()));

  return {
    projects,
    loading,
    loaded,
    creating,
    error,
    cloneRepo,
    load,
    create,
    remove,
    setAutoApprove,
  };
});
