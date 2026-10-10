// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { STATUS_POLL_MS, useChangesStore } from "./store";
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

  it("share sets prUrl and clears needsSignIn", async () => {
    const shareChange = vi.fn(async () => ({
      branch: "content/launch-week",
      title: "Launch week",
      prUrl: "https://github.com/acme/site/pull/1",
    }));
    const store = setup({ shareChange });
    store.active = { branch: "content/launch-week", title: "Launch week" };
    store.needsSignIn = true;

    const url = await store.share("site", "content/launch-week");
    expect(shareChange).toHaveBeenCalledWith("site", "content/launch-week");
    expect(url).toBe("https://github.com/acme/site/pull/1");
    expect(store.prUrl).toBe("https://github.com/acme/site/pull/1");
    expect(store.needsSignIn).toBe(false);
    expect(store.active?.prUrl).toBe("https://github.com/acme/site/pull/1");
  });

  it("share 403 surfaces push-access error", async () => {
    const store = setup({
      shareChange: vi.fn(async () => {
        throw new ApiError("SANDBOX", "You don't have push access", 403);
      }),
    });
    store.active = { branch: "content/x", title: "X" };
    await expect(store.share("site", "content/x")).rejects.toBeInstanceOf(ApiError);
    expect(store.error).toMatch(/push access/i);
    expect(store.needsSignIn).toBe(false);
  });

  it("share 401 sets needsSignIn without error text", async () => {
    const store = setup({
      shareChange: vi.fn(async () => {
        throw new ApiError("UNAUTH", "your GitHub session expired — sign in again", 401);
      }),
    });
    store.active = { branch: "content/x", title: "X" };
    await expect(store.share("site", "content/x")).rejects.toBeInstanceOf(ApiError);
    expect(store.needsSignIn).toBe(true);
    expect(store.error).toBeNull();
  });

  describe("PR status", () => {
    const draft = {
      number: 1,
      url: "https://github.com/acme/site/pull/1",
      state: "draft" as const,
      checks: "pending" as const,
      previewUrl: null,
    };

    async function withActive(changeStatus: ReturnType<typeof vi.fn>) {
      const store = setup({
        activeChange: vi.fn(async () => ({ branch: "content/lw", title: "LW" })),
        changeStatus,
      });
      await store.load("site");
      return store;
    }

    afterEach(() => {
      vi.useRealTimers();
    });

    it("load fetches the active change's PR status and its link", async () => {
      const changeStatus = vi.fn(async () => draft);
      const store = await withActive(changeStatus);
      expect(changeStatus).toHaveBeenCalledWith("site", "content/lw");
      expect(store.status).toEqual(draft);
      expect(store.prUrl).toBe(draft.url);
      store.stopStatusPolling();
    });

    it("polls while a PR exists", async () => {
      vi.useFakeTimers();
      const changeStatus = vi
        .fn()
        .mockResolvedValueOnce(draft)
        .mockResolvedValueOnce({ ...draft, state: "open", checks: "passing" });
      const store = await withActive(changeStatus);
      expect(changeStatus).toHaveBeenCalledTimes(1);
      await vi.advanceTimersByTimeAsync(STATUS_POLL_MS);
      expect(changeStatus).toHaveBeenCalledTimes(2);
      expect(store.status?.state).toBe("open");
      store.stopStatusPolling();
    });

    it("does not poll without a PR", async () => {
      vi.useFakeTimers();
      const changeStatus = vi.fn(async () => null);
      const store = await withActive(changeStatus);
      expect(store.status).toBeNull();
      await vi.advanceTimersByTimeAsync(STATUS_POLL_MS * 3);
      expect(changeStatus).toHaveBeenCalledTimes(1);
    });

    it("stops polling when the PR goes away or the change is left", async () => {
      vi.useFakeTimers();
      const changeStatus = vi.fn().mockResolvedValueOnce(draft).mockResolvedValue(null);
      const store = await withActive(changeStatus);
      await vi.advanceTimersByTimeAsync(STATUS_POLL_MS * 3);
      expect(changeStatus).toHaveBeenCalledTimes(2);
      expect(store.status).toBeNull();
    });

    it("share starts polling the new PR", async () => {
      vi.useFakeTimers();
      const changeStatus = vi.fn().mockResolvedValueOnce(null).mockResolvedValue(draft);
      const store = setup({
        activeChange: vi.fn(async () => ({ branch: "content/lw", title: "LW" })),
        shareChange: vi.fn(async () => ({
          branch: "content/lw",
          title: "LW",
          prUrl: draft.url,
        })),
        changeStatus,
      });
      await store.load("site");
      expect(store.status).toBeNull();
      await store.share("site", "content/lw");
      expect(store.status).toEqual(draft);
      await vi.advanceTimersByTimeAsync(STATUS_POLL_MS);
      expect(changeStatus).toHaveBeenCalledTimes(3);
      store.stopStatusPolling();
    });

    it("overlapping refreshes keep a single poll timer", async () => {
      vi.useFakeTimers();
      const changeStatus = vi.fn(async () => draft);
      const store = await withActive(changeStatus);
      await Promise.all([store.refreshStatus(), store.refreshStatus()]);
      const before = changeStatus.mock.calls.length;
      // Stopping must cancel every scheduled poll: no orphaned timer keeps polling.
      store.stopStatusPolling();
      await vi.advanceTimersByTimeAsync(STATUS_POLL_MS * 3);
      expect(changeStatus.mock.calls.length).toBe(before);
    });

    it("switchTo stops the old change's polling before switching", async () => {
      vi.useFakeTimers();
      const changeStatus = vi.fn(async () => draft);
      let finishSwitch!: (c: { branch: string; title: string }) => void;
      const store = setup({
        activeChange: vi.fn(async () => ({ branch: "content/lw", title: "LW" })),
        changeStatus,
        switchChange: vi.fn(
          () => new Promise((resolve) => (finishSwitch = resolve)),
        ),
        tree: vi.fn(async () => ({ entries: [] })),
      });
      await store.load("site");
      vi.spyOn(useFilesStore(), "refresh").mockResolvedValue();
      const switching = store.switchTo("site", "content/other");
      await vi.advanceTimersByTimeAsync(STATUS_POLL_MS * 2);
      expect(changeStatus).toHaveBeenCalledTimes(1);
      finishSwitch({ branch: "content/other", title: "Other" });
      await switching;
      expect(changeStatus).toHaveBeenLastCalledWith("site", "content/other");
      store.stopStatusPolling();
    });

    it("loading another project stops the previous project's polling", async () => {
      vi.useFakeTimers();
      const changeStatus = vi.fn(async () => draft);
      const activeChange = vi
        .fn()
        .mockResolvedValueOnce({ branch: "content/lw", title: "LW" })
        .mockReturnValueOnce(new Promise(() => {}));
      const store = setup({ activeChange, changeStatus });
      await store.load("site");
      void store.load("other");
      await vi.advanceTimersByTimeAsync(STATUS_POLL_MS * 2);
      expect(changeStatus).toHaveBeenCalledTimes(1);
      expect(store.status).toBeNull();
    });
  });
});

describe("changes store cleanup", () => {
  const merged = {
    number: 1,
    url: "https://github.com/acme/site/pull/1",
    state: "merged" as const,
    checks: "none" as const,
    previewUrl: null,
  };

  it("deletes the merged change, returns to main and drops it from the list", async () => {
    const cleanupChange = vi.fn(async () => ({ branch: "main" }));
    const store = setup({
      activeChange: vi.fn(async () => ({ branch: "content/lw", title: "LW" })),
      changeStatus: vi.fn(async () => merged),
      cleanupChange,
      tree: vi.fn(async () => ({ entries: [] })),
    });
    await store.load("site");
    store.items = [
      { branch: "content/lw", title: "LW" },
      { branch: "content/other", title: "Other" },
    ];
    const files = useFilesStore();
    files.projectId = "site";
    const refresh = vi.spyOn(files, "refresh").mockResolvedValue();
    const reload = vi.spyOn(usePreviewStore(), "reload");

    await store.cleanup("site", "content/lw");

    expect(cleanupChange).toHaveBeenCalledWith("site", "content/lw");
    expect(store.active).toBeNull();
    expect(store.status).toBeNull();
    expect(store.prUrl).toBeNull();
    expect(store.items.map((c) => c.branch)).toEqual(["content/other"]);
    expect(store.cleaningUp).toBe(false);
    expect(refresh).toHaveBeenCalled();
    expect(reload).toHaveBeenCalled();
  });

  it("keeps the change and surfaces the error when cleanup is refused", async () => {
    const store = setup({
      activeChange: vi.fn(async () => ({ branch: "content/lw", title: "LW" })),
      changeStatus: vi.fn(async () => merged),
      cleanupChange: vi.fn(async () => {
        throw new ApiError("CONFLICT", "This change has not been merged yet", 409);
      }),
    });
    await store.load("site");
    await expect(store.cleanup("site", "content/lw")).rejects.toThrow();
    expect(store.active?.branch).toBe("content/lw");
    expect(store.error).toContain("not been merged");
    expect(store.cleaningUp).toBe(false);
    store.stopStatusPolling();
  });
  it("stops status polling before the request and resumes it if cleanup fails", async () => {
    vi.useFakeTimers();
    try {
      const changeStatus = vi.fn(async () => merged);
      let refuse!: (e: unknown) => void;
      const store = setup({
        activeChange: vi.fn(async () => ({ branch: "content/lw", title: "LW" })),
        changeStatus,
        cleanupChange: vi.fn(
          () =>
            new Promise((_, reject) => {
              refuse = reject;
            }),
        ),
      });
      await store.load("site");
      expect(changeStatus).toHaveBeenCalledTimes(1);
      const cleaning = store.cleanup("site", "content/lw");
      await vi.advanceTimersByTimeAsync(STATUS_POLL_MS * 2);
      expect(changeStatus).toHaveBeenCalledTimes(1);
      refuse(new ApiError("CONFLICT", "not merged", 409));
      await expect(cleaning).rejects.toThrow();
      expect(changeStatus).toHaveBeenCalledTimes(2);
      store.stopStatusPolling();
    } finally {
      vi.useRealTimers();
    }
  });

  it("returns whether main was updated", async () => {
    const store = setup({
      cleanupChange: vi.fn(async () => ({ branch: "main", mainUpdated: false })),
      tree: vi.fn(async () => ({ entries: [] })),
    });
    vi.spyOn(useFilesStore(), "refresh").mockResolvedValue();
    await expect(store.cleanup("site", "content/lw")).resolves.toEqual({ mainUpdated: false });
  });
});
