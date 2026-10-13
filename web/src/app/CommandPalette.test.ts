// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn(), warning: vi.fn() } }));

import CommandPalette from "./CommandPalette.vue";
import { useProjectsStore } from "@/modules/projects/store";

const empty = { render: () => null };

async function setup(start: string) {
  h.daemon = createFakeDaemon({
    listContentCollections: vi.fn(async () => [{ name: "blog" }]),
    listContentEntries: vi.fn(async () => [{ collection: "blog", id: "launch-week", path: "src/content/blog/launch-week.md", kind: "markdown" }]),
    tree: vi.fn(async () => ({ entries: [{ kind: "file", name: "index.astro", path: "src/pages/index.astro" }] })),
  });
  const pinia = createPinia();
  setActivePinia(pinia);
  useProjectsStore().projects = [
    { id: "site", name: "Site", port: 1, devserver: "running", autoApprove: false, repo: null },
    { id: "docs", name: "Docs", port: 2, devserver: "running", autoApprove: false, repo: "acme/docs" },
  ] as never;
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: empty },
      { path: "/p/:id/:rest(.*)*", component: empty },
    ],
  });
  await router.push(start);
  const w = mount(CommandPalette, { attachTo: document.body, global: { plugins: [pinia, router] } });
  mounted = w;
  return { w, router };
}

let mounted: { unmount(): void } | null = null;
afterEach(() => {
  mounted?.unmount();
  mounted = null;
  document.body.innerHTML = "";
});

const press = (init: KeyboardEventInit) => window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", bubbles: true, ...init }));
const palette = () => document.querySelector('[data-testid="command-palette"]');
const item = (id: string) => document.querySelector<HTMLElement>(`[data-testid="command-${id}"]`);
async function type(text: string) {
  const input = document.querySelector<HTMLInputElement>('[data-testid="command-palette-input"]')!;
  input.value = text;
  input.dispatchEvent(new Event("input"));
  await flushPromises();
}

beforeEach(() => {
  localStorage.clear();
  vi.stubGlobal("matchMedia", () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
});

it("toggles with Ctrl/Cmd+K", async () => {
  await setup("/");
  press({ ctrlKey: true, metaKey: true });
  await flushPromises();
  expect(palette()).not.toBeNull();
  press({ ctrlKey: true, metaKey: true });
  await flushPromises();
  expect(palette()).toBeNull();
});

it("shows only global groups outside a project", async () => {
  await setup("/");
  press({ ctrlKey: true, metaKey: true });
  await flushPromises();
  expect(item("nav:content")).toBeNull();
  expect(item("action:get-latest")).toBeNull();
  expect(item("project:site")).not.toBeNull();
  expect(item("theme:dark")).not.toBeNull();
});

it("navigates to tabs, entries and files inside a project", async () => {
  const { router } = await setup("/p/site/preview");
  press({ ctrlKey: true, metaKey: true });
  await flushPromises();
  expect(item("project:site")).toBeNull();
  expect(item("entry:blog/launch-week")).toBeNull();

  item("nav:history")!.click();
  await flushPromises();
  expect(router.currentRoute.value.path).toBe("/p/site/history");

  press({ ctrlKey: true, metaKey: true });
  await flushPromises();
  await type("launch");
  item("entry:blog/launch-week")!.click();
  await flushPromises();
  expect(router.currentRoute.value.path).toBe("/p/site/content/blog/launch-week");

  press({ ctrlKey: true, metaKey: true });
  await flushPromises();
  await type("index");
  item("file:src/pages/index.astro")!.click();
  await flushPromises();
  expect(router.currentRoute.value.path).toBe("/p/site/code/src/pages/index.astro");
});

it("switches the theme", async () => {
  await setup("/");
  press({ ctrlKey: true, metaKey: true });
  await flushPromises();
  item("theme:dark")!.click();
  await flushPromises();
  expect(document.documentElement.classList.contains("dark")).toBe(true);
  expect(localStorage.getItem("workbench.theme")).toBe("dark");
});
