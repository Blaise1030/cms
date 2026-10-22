// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import ConfirmSwitchDialog from "./ConfirmSwitchDialog.vue";
import { useChangesStore } from "./store";
import { useChatStore } from "@/modules/chat/store";
import { useFilesStore } from "@/modules/files/store";
import { usePreviewStore } from "@/modules/preview/store";

function setup(api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return useChangesStore();
}

let wrapper: VueWrapper | null = null;
const body = () => new DOMWrapper(document.body);
const find = (testid: string) => body().find(`[data-testid='${testid}']`);
const click = async (testid: string) => {
  await find(testid).trigger("click");
  await flushPromises();
};

beforeEach(() => {
  vi.restoreAllMocks();
});

afterEach(() => {
  wrapper?.unmount();
  wrapper = null;
  document.body.innerHTML = "";
});

describe("ConfirmSwitchDialog", () => {
  it("renders the exact confirm copy with the old and new titles", async () => {
    const switchChange = vi.fn(async () => ({ branch: "content/new", title: "New" }));
    const store = setup({
      switchChange,
      stopChat: vi.fn(async () => ({ stopped: true })),
      tree: vi.fn(async () => ({ entries: [] })),
    });
    useChatStore().running.site = "c1";
    store.active = { branch: "content/old", title: "Old" };
    store.items = [{ branch: "content/new", title: "New" }];

    await store.switchTo("site", "content/new");

    wrapper = mount(ConfirmSwitchDialog, { attachTo: document.body });
    await flushPromises();

    expect(body().text()).toContain(
      "You switched drafts — this chat was on Old. Continue on New?",
    );
    expect(switchChange).not.toHaveBeenCalled();
  });

  it("cancel clears the pending switch without switching", async () => {
    const switchChange = vi.fn();
    const stopChat = vi.fn(async () => ({ stopped: true }));
    const store = setup({ switchChange, stopChat, tree: vi.fn(async () => ({ entries: [] })) });
    useChatStore().running.site = "c1";
    store.active = { branch: "content/old", title: "Old" };
    store.items = [{ branch: "content/new", title: "New" }];
    await store.switchTo("site", "content/new");

    wrapper = mount(ConfirmSwitchDialog, { attachTo: document.body });
    await flushPromises();
    await click("confirm-switch-cancel");

    expect(store.pendingSwitch).toBeNull();
    expect(switchChange).not.toHaveBeenCalled();
    expect(stopChat).not.toHaveBeenCalled();
    expect(store.active).toEqual({ branch: "content/old", title: "Old" });
  });

  it("continue stops the chat then switches", async () => {
    const switchChange = vi.fn(async () => ({ branch: "content/new", title: "New" }));
    const stopChat = vi.fn(async () => ({ stopped: true }));
    const store = setup({ switchChange, stopChat, tree: vi.fn(async () => ({ entries: [] })) });
    vi.spyOn(useFilesStore(), "refresh").mockResolvedValue();
    vi.spyOn(usePreviewStore(), "reload");
    useChatStore().running.site = "c1";
    store.active = { branch: "content/old", title: "Old" };
    store.items = [{ branch: "content/new", title: "New" }];
    await store.switchTo("site", "content/new");

    wrapper = mount(ConfirmSwitchDialog, { attachTo: document.body });
    await flushPromises();
    await click("confirm-switch-continue");

    expect(stopChat).toHaveBeenCalledWith("site");
    expect(switchChange).toHaveBeenCalledWith("site", "content/new");
    expect(store.active).toEqual({ branch: "content/new", title: "New" });
    expect(store.pendingSwitch).toBeNull();
  });
});
