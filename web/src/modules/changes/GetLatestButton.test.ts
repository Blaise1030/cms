// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { toast } from "vue-sonner";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
import type { ChangesUpdate } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn() } }));

import GetLatestButton from "./GetLatestButton.vue";
import { useChangesStore } from "./store";

function mountButton(updateChanges: () => Promise<ChangesUpdate>) {
  h.daemon = createFakeDaemon({ updateChanges, tree: vi.fn(async () => ({ entries: [] })) });
  setActivePinia(createPinia());
  return mount(GetLatestButton, { props: { projectId: "site" } });
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("GetLatestButton", () => {
  it("toasts Up to date on a clean update", async () => {
    const w = mountButton(async () => ({
      mainUpdated: true,
      mainDiverged: false,
      updated: [
        { branch: "content/a", title: "A" },
        { branch: "content/b", title: "B" },
      ],
      conflicts: [],
    }));
    await w.find("[data-testid='get-latest']").trigger("click");
    await flushPromises();
    expect(toast.success).toHaveBeenCalledWith("Up to date", {
      description: "Brought 2 changes up to date with main.",
    });
  });

  it("reports conflicts in N changes, naming them and what was updated", async () => {
    const w = mountButton(async () => ({
      mainUpdated: true,
      mainDiverged: false,
      updated: [{ branch: "content/b", title: "Pricing page" }],
      conflicts: [
        { branch: "content/a", title: "Launch week" },
        { branch: "content/c", title: "About us" },
      ],
    }));
    await w.find("[data-testid='get-latest']").trigger("click");
    await flushPromises();
    expect(toast.warning).toHaveBeenCalledWith("Conflicts in 2 changes", {
      description:
        "Left as they were: Launch week, About us. Updated main and 1 change (Pricing page).",
    });
    expect(toast.success).not.toHaveBeenCalled();
  });

  it("warns when local main has diverged", async () => {
    const w = mountButton(async () => ({
      mainUpdated: false,
      mainDiverged: true,
      updated: [{ branch: "content/b", title: "Pricing page" }],
      conflicts: [],
    }));
    await w.find("[data-testid='get-latest']").trigger("click");
    await flushPromises();
    expect(toast.warning).toHaveBeenCalledWith(
      "Changes updated, but main was left as is",
      expect.objectContaining({ description: expect.stringMatching(/not on GitHub/) }),
    );
  });

  it("is disabled while switching or creating a change", async () => {
    const w = mountButton(async () => ({
      mainUpdated: false,
      mainDiverged: false,
      updated: [],
      conflicts: [],
    }));
    const store = useChangesStore();
    store.switching = true;
    await flushPromises();
    expect(w.find("[data-testid='get-latest']").attributes("disabled")).toBeDefined();
    store.switching = false;
    store.creating = true;
    await flushPromises();
    expect(w.find("[data-testid='get-latest']").attributes("disabled")).toBeDefined();
    store.creating = false;
    await flushPromises();
    expect(w.find("[data-testid='get-latest']").attributes("disabled")).toBeUndefined();
  });

  it("shows the refusal message as an error toast", async () => {
    const w = mountButton(async () => {
      throw new ApiError("CONFLICT", "Uncommitted changes — checkpoint first", 409);
    });
    await w.find("[data-testid='get-latest']").trigger("click");
    await flushPromises();
    expect(toast.error).toHaveBeenCalledWith("Uncommitted changes — checkpoint first");
  });
});
