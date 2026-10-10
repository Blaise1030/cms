// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import ConflictView from "./ConflictView.vue";
import { useChangesStore } from "./store";

const session = {
  id: "s1", projectId: "site", branch: "content/page", tip: "a", incoming: "b", base: "c",
  expiresAt: 100, choices: {},
  files: [{ path: "src/content/sections/hero.mdx", binary: false, oursOid: "d", theirsOid: "e", baseOid: "f", patch: "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n" }],
};

beforeEach(() => {
  h.daemon = createFakeDaemon({
    getConflict: vi.fn(async () => session),
    chooseConflict: vi.fn(async () => ({ tree: "tree", complete: true })),
    finishConflict: vi.fn(async () => ({ commit: "merged" })),
    abandonConflict: vi.fn(async () => ({ abandoned: true })),
    tree: vi.fn(async () => ({ entries: [] })),
  });
  setActivePinia(createPinia());
});

describe("ConflictView", () => {
  it("shows file choices and enables Finish only after every file has a choice", async () => {
    const store = useChangesStore();
    await store.openConflict("site", "s1");
    const wrapper = mount(ConflictView, { props: { projectId: "site" } });
    await flushPromises();
    expect(wrapper.text()).toContain("hero.mdx");
    expect(wrapper.get("[data-testid='finish-conflict']").attributes("disabled")).toBeDefined();
    await wrapper.get("[data-testid='keep-yours']").trigger("click");
    await flushPromises();
    expect((h.daemon.api as unknown as Record<string, unknown>).chooseConflict).toHaveBeenCalledWith("site", "s1", "src/content/sections/hero.mdx", "yours");
    expect(wrapper.get("[data-testid='finish-conflict']").attributes("disabled")).toBeUndefined();
    await wrapper.get("[data-testid='finish-conflict']").trigger("click");
    await flushPromises();
    expect((h.daemon.api as unknown as Record<string, unknown>).finishConflict).toHaveBeenCalledWith("site", "s1");
    expect(store.selectedConflictId).toBeNull();
  });

  it("abandon leaves the branch alone and closes the view", async () => {
    const store = useChangesStore();
    await store.openConflict("site", "s1");
    const wrapper = mount(ConflictView, { props: { projectId: "site" } });
    await wrapper.get("[data-testid='abandon-conflict']").trigger("click");
    await wrapper.get("[data-testid='confirm-abandon']").trigger("click");
    await flushPromises();
    expect((h.daemon.api as unknown as Record<string, unknown>).abandonConflict).toHaveBeenCalledWith("site", "s1");
    expect(store.selectedConflictId).toBeNull();
  });

  it("keeps preview routes and widths aligned, and blocks a confirmed invalid result", async () => {
    const daemon = h.daemon.api as unknown as Record<string, unknown>;
    daemon.conflictPreviews = vi.fn(async () => ({
      yours: { status: "ready", url: "/preview/site/conflicts/s1/yours/" },
      latest: { status: "ready", url: "/preview/site/conflicts/s1/latest/" },
      result: { status: "invalid", errors: "Build failed" },
    }));
    const store = useChangesStore();
    await store.openConflict("site", "s1");
    store.currentConflict!.files[0]!.path = "src/content/sections/hero.mdx";
    store.candidate = { tree: "candidate", complete: true };
    const wrapper = mount(ConflictView, { props: { projectId: "site" } });
    await flushPromises();
    expect(wrapper.get("[data-testid='finish-conflict']").attributes("disabled")).toBeDefined();
    await wrapper.get('input[aria-label="Preview page"]').setValue("/about/");
    const frames = wrapper.findAll("iframe");
    expect(frames).toHaveLength(2);
    expect(frames[0]!.attributes("src")).toContain("/about/?section=hero");
    expect(frames[1]!.attributes("src")).toContain("/about/?section=hero");
    await wrapper.findAll("button").find((button) => button.text() === "Mobile")!.trigger("click");
    expect(wrapper.findAll("iframe")[0]!.element.parentElement?.getAttribute("style")).toContain("390px");
  });
});
