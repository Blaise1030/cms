// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { useFilesStore } from "@/modules/files/store";
import { useContentStore } from "@/modules/content/store";
import { useDesignStore } from "@/modules/design/store";
import { useChangesStore } from "@/modules/changes/store";
import { useSettingsStore } from "./store";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

beforeEach(() => { vi.restoreAllMocks(); });

describe("settings reload after a branch switch", () => {
  it("switchTo and create refresh the Settings tab for the new branch", async () => {
    h.daemon = createFakeDaemon({
      switchChange: vi.fn(async () => ({ branch: "content/alpha", title: "Alpha" })),
      createChange: vi.fn(async () => ({ branch: "content/beta", title: "Beta" })),
      tree: vi.fn(async () => ({ entries: [] })),
    });
    setActivePinia(createPinia());
    vi.spyOn(useFilesStore(), "refresh").mockResolvedValue();
    vi.spyOn(useContentStore(), "refreshForBranch").mockResolvedValue();
    vi.spyOn(useDesignStore(), "refreshForBranch").mockResolvedValue();
    const refreshForBranch = vi.spyOn(useSettingsStore(), "refreshForBranch").mockResolvedValue();
    const changes = useChangesStore();

    await changes.switchTo("site", "content/alpha");
    expect(refreshForBranch).toHaveBeenLastCalledWith("content/alpha");
    await changes.create("site", "Beta");
    expect(refreshForBranch).toHaveBeenLastCalledWith("content/beta");
  });
});
