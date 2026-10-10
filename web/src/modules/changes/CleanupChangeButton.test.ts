// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import type { PrStatus } from "@/core/types";
import CleanupChangeButton from "./CleanupChangeButton.vue";
import { useChangesStore } from "./store";

const h = vi.hoisted(() => ({ toastError: vi.fn(), toastWarning: vi.fn() }));
vi.mock("vue-sonner", () => ({ toast: { error: h.toastError, warning: h.toastWarning } }));

const merged: PrStatus = {
  number: 1,
  url: "https://github.com/acme/site/pull/1",
  state: "merged",
  checks: "none",
  previewUrl: null,
};

function mountWith(status: PrStatus | null) {
  setActivePinia(createPinia());
  const store = useChangesStore();
  store.active = { branch: "content/lw", title: "LW" };
  store.status = status;
  const wrapper = mount(CleanupChangeButton, { props: { projectId: "site" } });
  return { store, wrapper };
}

describe("CleanupChangeButton", () => {
  it("shows only once the active change's PR is merged", () => {
    expect(mountWith(null).wrapper.find('[data-testid="cleanup-change"]').exists()).toBe(false);
    for (const state of ["draft", "open", "approved", "closed"] as const) {
      const { wrapper } = mountWith({ ...merged, state });
      expect(wrapper.find('[data-testid="cleanup-change"]').exists()).toBe(false);
    }
    const { wrapper } = mountWith(merged);
    expect(wrapper.get('[data-testid="cleanup-change"]').text()).toBe("Clean up");
  });

  it("cleans up the active change on click", async () => {
    const { store, wrapper } = mountWith(merged);
    const cleanup = vi.spyOn(store, "cleanup").mockResolvedValue({ mainUpdated: true });
    await wrapper.get('[data-testid="cleanup-change"]').trigger("click");
    expect(cleanup).toHaveBeenCalledWith("site", "content/lw");
  });

  it("toasts when cleanup fails", async () => {
    const { store, wrapper } = mountWith(merged);
    vi.spyOn(store, "cleanup").mockRejectedValue(new Error("not merged yet"));
    await wrapper.get('[data-testid="cleanup-change"]').trigger("click");
    await flushPromises();
    expect(h.toastError).toHaveBeenCalledWith("not merged yet");
  });
  it("warns when main couldn't be updated", async () => {
    const { store, wrapper } = mountWith(merged);
    vi.spyOn(store, "cleanup").mockResolvedValue({ mainUpdated: false });
    await wrapper.get('[data-testid="cleanup-change"]').trigger("click");
    await flushPromises();
    expect(h.toastWarning).toHaveBeenCalledWith(expect.stringContaining("main"));
  });
});
