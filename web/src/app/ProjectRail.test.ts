// @vitest-environment happy-dom
import { expect, it } from "vitest";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import ProjectRail from "./ProjectRail.vue";

const Page = defineComponent({ props: { id: String }, setup: (props) => () => h(ProjectRail, { projectId: props.id! }) });
const empty = { render: () => null };

async function setup(start: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: empty },
      { path: "/p/:id", component: Page, props: true, children: [
        { path: "preview", name: "project-preview", component: empty },
        { path: "content/:rest(.*)?", name: "project-content", component: empty },
        { path: "code/:rest(.*)?", name: "project-code", component: empty },
        { path: "history", name: "project-history", component: empty },
      ] },
    ],
  });
  await router.push(start);
  const w = mount(RouterView, { global: { plugins: [router] } });
  await flushPromises();
  return { w, router };
}

it("links each tab and marks the current one, including nested pages", async () => {
  const { w, router } = await setup("/p/my%20site/content/blog/welcome");
  expect(w.get('[data-testid="workspace-tab-content"]').attributes("aria-current")).toBe("page");
  expect(w.get('[data-testid="workspace-tab-preview"]').attributes("aria-current")).toBeUndefined();
  expect(w.get('[data-testid="workspace-tab-code"]').attributes("href")).toBe("/p/my%20site/code");

  await w.get('[data-testid="workspace-tab-history"]').trigger("click"); await flushPromises();
  expect(router.currentRoute.value.name).toBe("project-history");
  expect(w.get('[data-testid="workspace-tab-history"]').attributes("aria-current")).toBe("page");

  await w.get('[data-testid="project-rail-home"]').trigger("click"); await flushPromises();
  expect(router.currentRoute.value.path).toBe("/");
});

it("keeps its buttons below the macOS traffic lights and drags the window from above them", async () => {
  const rail = async () => (await setup("/p/site/preview")).w.get('[data-testid="project-rail"]');

  const browser = await rail();
  expect(browser.attributes("data-tauri-drag-region")).toBeUndefined();
  expect(browser.classes()).not.toContain("pt-11");

  document.documentElement.dataset.desktop = "macos";
  try {
    const desktop = await rail();
    expect(desktop.attributes("data-tauri-drag-region")).toBe("deep");
    expect(desktop.classes()).toContain("pt-11");
  } finally {
    delete document.documentElement.dataset.desktop;
  }
});
