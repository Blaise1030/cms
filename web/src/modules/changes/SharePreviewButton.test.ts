// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { TooltipProvider } from "reka-ui";
import { defineComponent, h } from "vue";
import SharePreviewButton from "./SharePreviewButton.vue";
import { useChangesStore } from "./store";

vi.mock("@/lib/toast", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

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
    mergeable: true,
    mergeableState: "clean",
    upToDate: false,
  };
  // UTooltip needs a TooltipProvider; `UApp` supplies one in the app.
  const Host = defineComponent(
    () => () => h(TooltipProvider, null, () => h(SharePreviewButton, { projectId: "site", repo })),
  );
  const wrapper = mount(Host, {
    global: {
      stubs: { PrMenu: true, ProtectionHint: true },
    },
  });
  return { store, wrapper };
}

describe("SharePreviewButton", () => {
  it("shows Publish as its own button beside the share group", () => {
    const { wrapper } = mountWith();
    const group = wrapper.get('[data-testid="share-group"]');
    expect(group.find('[data-testid="share-preview"]').exists()).toBe(true);
    expect(group.find('[data-testid="publish-change"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="publish-change"]').exists()).toBe(true);
  });

  it("omits Publish from the group for local projects", () => {
    const { wrapper } = mountWith(null);
    expect(wrapper.find('[data-testid="share-group"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="share-preview"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="publish-change"]').exists()).toBe(false);
  });

  it("hides Update preview once the branch matches the pull request", async () => {
    const { store, wrapper } = mountWith();
    store.prUrl = "https://github.com/acme/site/pull/1";
    store.status = { ...store.status!, upToDate: true };
    await flushPromises();
    expect(wrapper.find('[data-testid="share-preview"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="publish-change"]').exists()).toBe(true);
  });

  it("keeps Update preview while there are local commits to push", async () => {
    const { store, wrapper } = mountWith();
    store.prUrl = "https://github.com/acme/site/pull/1";
    await flushPromises();
    expect(wrapper.find('[data-testid="share-preview"]').exists()).toBe(true);
  });

  it("links the live site on main when a production URL is known", async () => {
    setActivePinia(createPinia());
    const store = useChangesStore();
    store.active = null;
    store.liveUrl = "https://acme.pages.dev";
    const wrapper = mount(SharePreviewButton, {
      props: { projectId: "site", repo: "acme/site" },
      global: { stubs: { PrMenu: true, ProtectionHint: true } },
    });
    const a = wrapper.get('[data-testid="live-site-link"]');
    expect(a.attributes("href")).toBe("https://acme.pages.dev");
    expect(a.text()).toContain("Open live site");
  });

  it("hides the live site link without a production URL or while on a change", async () => {
    const { store, wrapper } = mountWith();
    store.liveUrl = "https://acme.pages.dev";
    await flushPromises();
    expect(wrapper.find('[data-testid="live-site-link"]').exists()).toBe(false);

    setActivePinia(createPinia());
    const onMain = useChangesStore();
    onMain.active = null;
    onMain.liveUrl = null;
    const empty = mount(SharePreviewButton, {
      props: { projectId: "site", repo: "acme/site" },
      global: { stubs: { PrMenu: true, ProtectionHint: true } },
    });
    expect(empty.find('[data-testid="live-site-link"]').exists()).toBe(false);
  });

  it("disables Share preview and Publish until the new change has an edit", async () => {
    const { store, wrapper } = mountWith();
    store.status = null;
    store.prUrl = null;
    store.hasEdits = false;
    await flushPromises();
    const share = wrapper.get('[data-testid="share-preview"]');
    expect(share.attributes("disabled")).toBeDefined();
    expect(share.attributes("aria-description")).toMatch(/Make an edit first/);
    expect(wrapper.get('[data-testid="publish-change"]').attributes("disabled")).toBeDefined();

    store.hasEdits = true;
    await flushPromises();
    expect(wrapper.get('[data-testid="share-preview"]').attributes("disabled")).toBeUndefined();
    expect(wrapper.get('[data-testid="publish-change"]').attributes("disabled")).toBeUndefined();
  });

  it("keeps Update preview open once a pull request exists", async () => {
    const { store, wrapper } = mountWith();
    store.prUrl = "https://github.com/acme/site/pull/1";
    store.hasEdits = false;
    await flushPromises();
    const share = wrapper.get('[data-testid="share-preview"]');
    expect(share.attributes("disabled")).toBeUndefined();
    // A direct child of the group, so its corners join Review's.
    expect(share.element.parentElement?.dataset.testid).toBe("share-group");
  });
});
