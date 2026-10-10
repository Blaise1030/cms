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
      updated: ["content/a", "content/b"],
      conflicts: [],
    }));
    await w.find("[data-testid='get-latest']").trigger("click");
    await flushPromises();
    expect(toast.success).toHaveBeenCalledWith("Up to date", {
      description: "Brought 2 changes up to date with main.",
    });
  });

  it("reports conflicts in N changes", async () => {
    const w = mountButton(async () => ({
      mainUpdated: true,
      updated: [],
      conflicts: ["content/a"],
    }));
    await w.find("[data-testid='get-latest']").trigger("click");
    await flushPromises();
    expect(toast.warning).toHaveBeenCalledWith("Conflicts in 1 change", expect.anything());
    expect(toast.success).not.toHaveBeenCalled();
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
