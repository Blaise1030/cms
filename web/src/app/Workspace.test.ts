// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { defineComponent, h } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
vi.mock("@/modules/changes/store", () => ({ useChangesStore: () => ({ selectedConflictId: null }) }));
import Workspace from "./Workspace.vue";

const empty = { render: () => null };
const PreviewStub = defineComponent({
  name: "PreviewPanel",
  inheritAttrs: false,
  setup(_, { attrs }) {
    return () => h("div", { "data-testid": "preview-panel", ...attrs });
  },
});

const project = {
  id: "site",
  name: "Site",
  port: 4321,
  devserver: "ready" as const,
  autoApprove: false,
  repo: null,
};

async function setup(start: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/p/:id/preview", name: "project-preview", component: empty },
      {
        path: "/p/:id/content",
        name: "project-content",
        component: { template: '<div data-testid="content-surface" />' },
      },
    ],
  });
  await router.push(start);
  await router.isReady();
  const w = mount(Workspace, {
    props: { project },
    global: { plugins: [router], stubs: { PreviewPanel: PreviewStub } },
  });
  await flushPromises();
  return { w, router };
}

describe("Workspace", () => {
  it("keeps the preview laid out instead of hiding it with display:none", async () => {
    const { w, router } = await setup("/p/site/preview");
    const preview = w.get('[data-testid="preview-panel"]');
    const surface = w.get('[data-testid="workspace-surface"]');
    expect(preview.classes()).not.toContain("invisible");
    expect(preview.attributes("inert")).toBeUndefined();
    expect((surface.element as HTMLElement).style.display).toBe("none");

    await router.push("/p/site/content");
    await flushPromises();
    expect(preview.classes()).toContain("invisible");
    expect(preview.attributes("inert")).toBeDefined();
    expect((preview.element as HTMLElement).style.display).not.toBe("none");
    expect((surface.element as HTMLElement).style.display).not.toBe("none");

    await router.push("/p/site/preview");
    await flushPromises();
    expect(preview.classes()).not.toContain("invisible");
    expect((surface.element as HTMLElement).style.display).toBe("none");
    w.unmount();
  });

  it("pads the workspace only when the floating chat is shown", async () => {
    const { w } = await setup("/p/site/preview");
    expect(w.get("[data-testid='workspace-root']").classes()).not.toContain("pb-11");

    await w.setProps({ floatingChat: true });
    await flushPromises();
    expect(w.get("[data-testid='workspace-root']").classes()).toContain("pb-11");

    await w.setProps({ floatingChat: false });
    await flushPromises();
    expect(w.get("[data-testid='workspace-root']").classes()).not.toContain("pb-11");
    w.unmount();
  });
});
