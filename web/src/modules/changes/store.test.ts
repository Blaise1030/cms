// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useChangesStore } from "./store";
import { useFilesStore } from "@/modules/files/store";
import { usePreviewStore } from "@/modules/preview/store";

function setup(api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return useChangesStore();
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("changes store", () => {
  it("sets active change after create", async () => {
    const createChange = vi.fn(async () => ({
      branch: "content/launch-week",
      title: "Launch week",
    }));
    const store = setup({ createChange, tree: vi.fn(async () => ({ entries: [] })) });
    await store.create("site", "New post: Launch week");
    expect(createChange).toHaveBeenCalledWith("site", "New post: Launch week");
    expect(store.active).toEqual({ branch: "content/launch-week", title: "Launch week" });
    expect(store.projectId).toBe("site");
  });

  it("create refreshes files and preview", async () => {
    const store = setup({
      createChange: vi.fn(async () => ({ branch: "content/x", title: "X" })),
      tree: vi.fn(async () => ({ entries: [] })),
    });
    const files = useFilesStore();
    files.projectId = "site";
    const refresh = vi.spyOn(files, "refresh").mockResolvedValue();
    const preview = usePreviewStore();
    const reload = vi.spyOn(preview, "reload");

    await store.create("site", "X");
    expect(refresh).toHaveBeenCalled();
    expect(reload).toHaveBeenCalled();
  });

  it("list loads open changes", async () => {
    const listChanges = vi.fn(async () => [
      { branch: "content/newer", title: "Newer", lastEdited: 200 },
      { branch: "content/older", title: "Older", lastEdited: 100 },
    ]);
    const store = setup({ listChanges });
    await store.list("site");
    expect(listChanges).toHaveBeenCalledWith("site");
    expect(store.items.map((c) => c.branch)).toEqual(["content/newer", "content/older"]);
  });

  it("switchTo sets active and refreshes tree/preview", async () => {
    const switchChange = vi.fn(async () => ({
      branch: "content/alpha",
      title: "Alpha",
    }));
    const store = setup({
      switchChange,
      tree: vi.fn(async () => ({ entries: [] })),
    });
    const files = useFilesStore();
    files.projectId = "site";
    const refresh = vi.spyOn(files, "refresh").mockResolvedValue();
    const preview = usePreviewStore();
    const reload = vi.spyOn(preview, "reload");

    await store.switchTo("site", "content/alpha");
    expect(switchChange).toHaveBeenCalledWith("site", "content/alpha");
    expect(store.active).toEqual({ branch: "content/alpha", title: "Alpha" });
    expect(refresh).toHaveBeenCalled();
    expect(reload).toHaveBeenCalled();
  });

  it("switchTo surfaces checkpoint-first errors", async () => {
    const store = setup({
      switchChange: vi.fn(async () => {
        throw new ApiError("CONFLICT", "Uncommitted changes — checkpoint first", 409);
      }),
    });
    await expect(store.switchTo("site", "content/alpha")).rejects.toBeInstanceOf(ApiError);
    expect(store.error).toMatch(/checkpoint/i);
  });

  it("load sets active from GET /api/changes/active", async () => {
    const activeChange = vi.fn(async () => ({
      branch: "content/launch-week",
      title: "Launch week",
    }));
    const store = setup({ activeChange });
    await store.load("site");
    expect(activeChange).toHaveBeenCalledWith("site");
    expect(store.active?.title).toBe("Launch week");
  });

  it("load clears active when none (ENOENT)", async () => {
    const store = setup({
      activeChange: vi.fn(async () => {
        throw new ApiError("ENOENT", "no active change", 404);
      }),
    });
    store.active = { branch: "content/old", title: "Old" };
    await store.load("site");
    expect(store.active).toBeNull();
    expect(store.error).toBeNull();
  });

  it("ignores stale load responses after project switch", async () => {
    let resolveA!: (v: { branch: string; title: string }) => void;
    const a = new Promise<{ branch: string; title: string }>((r) => {
      resolveA = r;
    });
    const activeChange = vi.fn(async (id: string) => {
      if (id === "a") return a;
      return { branch: "content/b", title: "Project B" };
    });
    const store = setup({ activeChange });

    const loadA = store.load("a");
    await store.load("b");
    expect(store.active?.title).toBe("Project B");

    resolveA({ branch: "content/a", title: "Project A" });
    await loadA;
    expect(store.active?.title).toBe("Project B");
    expect(store.projectId).toBe("b");
  });

  it("clears active when starting load for a new project", async () => {
    let resolveB!: (v: { branch: string; title: string }) => void;
    const b = new Promise<{ branch: string; title: string }>((r) => {
      resolveB = r;
    });
    const activeChange = vi.fn(async (id: string) => {
      if (id === "a") return { branch: "content/a", title: "Project A" };
      return b;
    });
    const store = setup({ activeChange });
    await store.load("a");
    expect(store.active?.title).toBe("Project A");

    const loadB = store.load("b");
    expect(store.active).toBeNull();
    resolveB({ branch: "content/b", title: "Project B" });
    await loadB;
    expect(store.active?.title).toBe("Project B");
  });

  it("error does not leave a foreign active title", async () => {
    const store = setup({
      activeChange: vi.fn(async () => {
        throw new ApiError("GIT_ERR", "git failed", 500);
      }),
    });
    store.active = { branch: "content/foreign", title: "Foreign" };
    await store.load("site");
    expect(store.active).toBeNull();
    expect(store.error).toMatch(/git failed/i);
  });
});
