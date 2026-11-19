// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon } from "@/test/fakeDaemon";
import { useReviewStore } from "@/modules/review/store";

const hoisted = vi.hoisted(() => ({ daemon: null as unknown }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => hoisted.daemon }));

import ProjectRail from "./ProjectRail.vue";

const Page = defineComponent({ props: { id: String }, setup: (props) => () => h(ProjectRail, { projectId: props.id! }) });
const empty = { render: () => null };

async function setup(start: string, waiting = 0) {
  hoisted.daemon = createFakeDaemon({
    reviews: vi.fn(async () =>
      Array.from({ length: waiting }, (_, i) => ({ number: i + 1, myState: "new" })).concat([
        { number: 99, myState: "greenlit" },
      ]),
    ),
  });
  setActivePinia(createPinia());
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: empty },
      { path: "/settings", component: empty },
      { path: "/p/:id", component: Page, props: true, children: [
        { path: "preview", name: "project-preview", component: empty },
        { path: "content/:rest(.*)?", name: "project-content", component: empty },
        { path: "code/:rest(.*)?", name: "project-code", component: empty },
        { path: "history", name: "project-history", component: empty },
        { path: "review/:number?", name: "project-review", component: empty },
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

it("renders Settings as the last tab, linking to the project's settings", async () => {
  const { w } = await setup("/p/my%20site/preview");
  // Review lives in the footer, so it is left out here.
  const tabs = w.findAll('[data-testid^="workspace-tab-"]:not([data-testid="workspace-tab-review"])');
  expect(tabs.at(-1)!.attributes("data-testid")).toBe("workspace-tab-settings");
  expect(tabs.at(-1)!.attributes("href")).toBe("/p/my%20site/settings");
  expect(tabs.at(-1)!.attributes("aria-label")).toBe("Settings");
});

it("links to app settings from the rail footer, apart from the project tabs", async () => {
  const { w, router } = await setup("/p/my%20site/preview");
  const link = w.get('[data-testid="project-rail-app-settings"]');
  expect(link.attributes("href")).toBe("/settings");
  expect(link.attributes("aria-label")).toBe("App settings");
  expect(w.findAll('[data-testid^="workspace-tab-"]')).toHaveLength(7);
  await link.trigger("click");
  await flushPromises();
  expect(router.currentRoute.value.path).toBe("/settings");
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

it("shows the Review tab with a badge of changes waiting for you", async () => {
  const { w } = await setup("/p/site/preview", 2);
  // In the footer group, directly above App settings.
  const footer = w.get('[data-testid="project-rail-app-settings"]').element.closest("div.mt-auto")!;
  const buttons = [...footer.querySelectorAll("[data-testid]")].map((el) => el.getAttribute("data-testid"));
  expect(buttons.slice(0, 3)).toEqual(["workspace-tab-review", "review-badge", "project-rail-app-settings"]);
  expect(w.get('[data-testid="workspace-tab-review"]').attributes("href")).toBe("/p/site/review");
  expect(w.get('[data-testid="workspace-tab-review"]').attributes("aria-label")).toBe("Review");
  expect(useReviewStore().waitingCount).toBe(2);
  expect(w.get('[data-testid="review-badge"]').text()).toBe("2");
});

it("hides the Review badge when nothing is waiting", async () => {
  const { w } = await setup("/p/site/preview", 0);
  expect(w.find('[data-testid="workspace-tab-review"]').exists()).toBe(true);
  expect(w.find('[data-testid="review-badge"]').exists()).toBe(false);
});

it("reloads the review inbox when the project changes", async () => {
  const { w, router } = await setup("/p/one/preview", 1);
  const reviews = (hoisted.daemon as { api: { reviews: ReturnType<typeof vi.fn> } }).api.reviews;
  expect(reviews).toHaveBeenLastCalledWith("one");
  await router.push("/p/two/preview"); await flushPromises();
  expect(reviews).toHaveBeenLastCalledWith("two");
  w.unmount();
});
