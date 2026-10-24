// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import SharePreviewButton from "./SharePreviewButton.vue";
import { useChangesStore } from "./store";

vi.mock("vue-sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

function mountWith(repo: string | null = "acme/site") {
  setActivePinia(createPinia());
  const store = useChangesStore();
  store.active = { branch: "content/lw", title: "LW" };
  store.status = {
    number: 1,
    url: "https://github.com/acme/site/pull/1",
    state: "open",
    checks: "none",
    previewUrl: null,
  };
  const wrapper = mount(SharePreviewButton, {
    props: { projectId: "site", repo },
    global: {
      stubs: { PrMenu: true, ProtectionHint: true },
    },
  });
  return { store, wrapper };
}

describe("SharePreviewButton", () => {
  it("groups Share preview and Publish in one button group", () => {
    const { wrapper } = mountWith();
    const group = wrapper.get('[data-testid="share-publish-group"]');
    expect(group.find('[data-testid="share-preview"]').exists()).toBe(true);
    expect(group.find('[data-testid="publish-change"]').exists()).toBe(true);
  });

  it("omits Publish from the group for local projects", () => {
    const { wrapper } = mountWith(null);
    expect(wrapper.find('[data-testid="share-publish-group"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="share-preview"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="publish-change"]').exists()).toBe(false);
  });
});
