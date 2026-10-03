// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { Project } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useProjectsStore } from "./store";

const project = (id: string, over: Partial<Project> = {}): Project => ({
  id,
  name: id,
  port: 4000,
  devserver: "ready",
  autoApprove: false,
  repo: null,
  ...over,
});

function setup(api: Record<string, (...a: never[]) => unknown>) {
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return useProjectsStore();
}

beforeEach(() => localStorage.clear());

describe("projects store", () => {
  it("loads the project list", async () => {
    const store = setup({ listProjects: vi.fn(async () => [project("a"), project("b")]) });
    await store.load();
    expect(store.projects.map((p) => p.id)).toEqual(["a", "b"]);
    expect(store.loaded).toBe(true);
    expect(store.error).toBeNull();
  });

  it("records a load error", async () => {
    const store = setup({
      listProjects: vi.fn(async () => {
        throw new Error("boom");
      }),
    });
    await store.load();
    expect(store.error).toBe("boom");
    expect(store.loaded).toBe(false);
  });

  it("creates a project with a trimmed name", async () => {
    const createProject = vi.fn(async (name: string) => project("my-site", { name }));
    const store = setup({ createProject });
    const created = await store.create("  My Site ");
    expect(createProject).toHaveBeenCalledWith("My Site");
    expect(created.id).toBe("my-site");
    expect(store.projects).toHaveLength(1);
    expect(store.projects[0]!.name).toBe("My Site");
  });

  it("rejects an empty name without calling the daemon", async () => {
    const createProject = vi.fn();
    const store = setup({ createProject });
    await expect(store.create("   ")).rejects.toThrow(/required/);
    expect(createProject).not.toHaveBeenCalled();
  });

  it("removes a project", async () => {
    const deleteProject = vi.fn(async () => ({ deleted: "a" }));
    const store = setup({ listProjects: async () => [project("a"), project("b")], deleteProject });
    await store.load();
    await store.remove("a");
    expect(deleteProject).toHaveBeenCalledWith("a");
    expect(store.projects.map((p) => p.id)).toEqual(["b"]);
  });

  it("applies devserver.status events and reloads after reconnect", async () => {
    const listProjects = vi.fn(async () => [project("a", { devserver: "starting", port: 1 })]);
    const store = setup({ listProjects });
    await store.load();
    h.daemon.emit({ type: "devserver.status", projectId: "a", state: "ready", port: 4321 });
    expect(store.projects[0]).toMatchObject({ devserver: "ready", port: 4321 });
    h.daemon.simulateReconnect();
    expect(listProjects).toHaveBeenCalledTimes(2);
  });

  it("reloads the list when the server reports events.lagged", async () => {
    const listProjects = vi.fn(async () => [project("a")]);
    setup({ listProjects });
    h.daemon.emit({ type: "events.lagged", missed: 5 });
    await vi.waitFor(() => expect(listProjects).toHaveBeenCalledTimes(1));
  });

  it("patches autoApprove", async () => {
    const updateProject = vi.fn(async (id: string) => project(id, { autoApprove: true }));
    const store = setup({ listProjects: async () => [project("a")], updateProject });
    await store.load();
    await store.setAutoApprove("a", true);
    expect(updateProject).toHaveBeenCalledWith("a", { autoApprove: true });
    expect(store.projects[0]!.autoApprove).toBe(true);
  });
});
