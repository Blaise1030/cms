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

  it("speaks in pages and versions, never git terms or paths up front", async () => {
    const store = useChangesStore();
    await store.openConflict("site", "s1");
    const wrapper = mount(ConflictView, { props: { projectId: "site" } });
    await flushPromises();
    expect(wrapper.text()).toContain("You and a teammate both changed the Hero section.");
    expect(wrapper.get("[data-testid='conflict-item']").text()).toContain("Hero section");
    expect(wrapper.get("[data-testid='conflict-item']").text()).toContain("Needs a decision");
    expect(wrapper.get("[data-testid='conflict-progress']").text()).toBe("0 of 1 decided");
    expect(wrapper.get("[data-testid='finish-conflict']").attributes("title")).toBe("Choose a version for every part first.");
    // Everything outside the collapsed code block avoids the implementation's words.
    const visible = wrapper.text().replace(wrapper.get("[data-testid='conflict-code']").text(), "");
    expect(visible).not.toMatch(/\bmain\b|branch|merge|commit|conflict|content\/page|src\//i);

    await wrapper.get("[data-testid='use-latest']").trigger("click");
    await flushPromises();
    expect(wrapper.get("[data-testid='conflict-progress']").text()).toBe("1 of 1 decided");
    expect(wrapper.get("[data-testid='conflict-item']").text()).toContain("Live site version");
    expect(wrapper.get("[data-testid='use-latest']").attributes("aria-pressed")).toBe("true");
    expect(wrapper.get("[data-testid='keep-yours']").attributes("aria-pressed")).toBe("false");
  });

  it("offers Keep both only when both sides are text that still exists", async () => {
    const store = useChangesStore();
    await store.openConflict("site", "s1");
    store.currentConflict!.files[0]!.oursDeleted = true;
    const wrapper = mount(ConflictView, { props: { projectId: "site" } });
    await flushPromises();
    expect(wrapper.get("[data-testid='keep-yours']").text()).toBe("Keep your deletion");
    expect(wrapper.get("[data-testid='keep-both']").attributes("disabled")).toBeDefined();
    expect(wrapper.text()).toContain("You removed this, but it was changed on the live site since.");
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
      yours: { status: "ready", port: 5001, token: "ty" },
      latest: { status: "ready", port: 5002, token: "tl" },
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
    // Each side at the root of its own origin (the daemon's host), first load with its token.
    expect(frames[0]!.attributes("src")).toBe("http://127.0.0.1:5001/about/?t=ty&section=hero#hero");
    expect(frames[1]!.attributes("src")).toBe("http://127.0.0.1:5002/about/?t=tl&section=hero#hero");
    await wrapper.findAll("button").find((button) => button.text() === "Mobile")!.trigger("click");
    expect(wrapper.findAll("iframe")[0]!.element.parentElement?.getAttribute("style")).toContain("390px");
  });

  it("warns and blocks Finish when the selected result page is missing", async () => {
    const daemon = h.daemon.api as unknown as Record<string, unknown>;
    daemon.conflictPreviews = vi.fn(async () => ({
      yours: { status: "ready", port: 5001, token: "ty" },
      latest: { status: "ready", port: 5002, token: "tl" },
      result: { status: "ready", port: 5003, token: "tr" },
    }));
    const store = useChangesStore();
    await store.openConflict("site", "s1");
    store.candidate = { tree: "candidate", complete: true };
    // The frames are cross-origin: whether a page exists comes from the daemon.
    const conflictPreviewPage = vi.fn(async (_p: string, _id: string, _source: string, path: string) => ({
      found: path === "/about/",
    }));
    daemon.conflictPreviewPage = conflictPreviewPage;
    const wrapper = mount(ConflictView, { props: { projectId: "site" } });
    await flushPromises();
    expect(wrapper.get("[data-testid='finish-conflict']").attributes("disabled")).toBeDefined();
    const frame = wrapper.get('iframe[title="result preview"]');
    await frame.trigger("load");
    await flushPromises();
    expect(conflictPreviewPage).toHaveBeenCalledWith("site", "s1", "result", "/");
    expect(wrapper.text()).toContain("This page is missing from the preview");
    expect(wrapper.get("[data-testid='finish-conflict']").attributes("disabled")).toBeDefined();
    await wrapper.get('input[aria-label="Preview page"]').setValue("/about/");
    await flushPromises();
    expect(wrapper.get("[data-testid='finish-conflict']").attributes("disabled")).toBeDefined();
    const nextFrame = wrapper.get('iframe[title="result preview"]');
    await nextFrame.trigger("load");
    await flushPromises();
    expect(wrapper.get("[data-testid='finish-conflict']").attributes("disabled")).toBeUndefined();
  });
});
