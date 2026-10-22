// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import type { PrStatus } from "@/core/types";
import PublishButton from "./PublishButton.vue";
import { useChangesStore } from "./store";

const h = vi.hoisted(() => ({ toastError: vi.fn(), toastSuccess: vi.fn() }));
vi.mock("vue-sonner", () => ({ toast: { error: h.toastError, success: h.toastSuccess } }));

const open: PrStatus = {
  number: 1,
  url: "https://github.com/acme/site/pull/1",
  state: "open",
  checks: "none",
  previewUrl: null,
};
const merged: PrStatus = { ...open, state: "merged" };

function mountWith(opts: { status?: PrStatus | null; repo?: string | null } = {}) {
  setActivePinia(createPinia());
  const store = useChangesStore();
  if (opts.status !== null) store.status = opts.status ?? open;
  store.active = { branch: "content/lw", title: "LW" };
  const wrapper = mount(PublishButton, {
    props: { projectId: "site", repo: "repo" in opts ? opts.repo : "acme/site" },
  });
  return { store, wrapper };
}

describe("PublishButton", () => {
  it("shows on a GitHub-backed active change that isn't live yet", () => {
    const { wrapper } = mountWith();
    expect(wrapper.get('[data-testid="publish-change"]').text()).toBe("Publish");
  });

  it("is hidden for template projects without a GitHub repo", () => {
    const { wrapper } = mountWith({ repo: null });
    expect(wrapper.find('[data-testid="publish-change"]').exists()).toBe(false);
  });

  it("is hidden once the change is live", () => {
    const { wrapper } = mountWith({ status: merged });
    expect(wrapper.find('[data-testid="publish-change"]').exists()).toBe(false);
  });

  it("publishes the active change on click and toasts that it is live", async () => {
    const { store, wrapper } = mountWith();
    const publish = vi.spyOn(store, "publish").mockResolvedValue();
    await wrapper.get('[data-testid="publish-change"]').trigger("click");
    await flushPromises();
    expect(publish).toHaveBeenCalledWith("site", "content/lw");
    expect(h.toastSuccess).toHaveBeenCalledWith("It's live.");
    expect(h.toastError).not.toHaveBeenCalled();
  });

  it("shows the in-flight state while publishing", async () => {
    const { store, wrapper } = mountWith();
    store.publishing = true;
    await flushPromises();
    const button = wrapper.get('[data-testid="publish-change"]');
    expect(button.attributes("disabled")).toBeDefined();
    expect(button.text()).toBe("Publishing…");
  });

  it("toasts the failure message without the live copy", async () => {
    const { store, wrapper } = mountWith();
    vi.spyOn(store, "publish").mockRejectedValue(new Error("Ask an owner to add you to this site."));
    await wrapper.get('[data-testid="publish-change"]').trigger("click");
    await flushPromises();
    expect(h.toastError).toHaveBeenCalledWith("Ask an owner to add you to this site.");
    expect(h.toastSuccess).not.toHaveBeenCalled();
  });
});
