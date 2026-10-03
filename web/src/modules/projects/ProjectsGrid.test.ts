// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter, type Router } from "vue-router";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { Project } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

import ProjectsGrid from "./ProjectsGrid.vue";

const p = (id: string, devserver: Project["devserver"]): Project => ({
  id,
  name: `Site ${id}`,
  port: 4000,
  devserver,
  autoApprove: false,
  repo: null,
});

let api: Record<string, ReturnType<typeof vi.fn>>;
let router: Router;

function mountGrid() {
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: { template: "<div />" } },
      { path: "/p/:id", component: { template: "<div />" } },
    ],
  });
  return mount(ProjectsGrid, { attachTo: document.body, global: { plugins: [router] } });
}

const byText = (text: string) =>
  [...document.body.querySelectorAll("button")].find((b) => b.textContent?.includes(text));

async function chooseDelete(w: ReturnType<typeof mountGrid>, name: string) {
  await w.find(`[aria-label='Actions for ${name}']`).trigger("click");
  await flushPromises();
  const item = [...document.body.querySelectorAll("[role='menuitem']")].find((m) =>
    m.textContent?.includes("Delete"),
  ) as HTMLElement;
  item.click();
  await flushPromises();
}

beforeEach(() => {
  localStorage.clear();
  api = {
    listProjects: vi.fn(async () => [p("a", "ready"), p("b", "crashed")]),
    createProject: vi.fn(async (name: string) => ({ ...p("new", "starting"), name })),
    deleteProject: vi.fn(async (id: string) => ({ deleted: id })),
  };
});
afterEach(() => {
  document.body.innerHTML = "";
});

describe("ProjectsGrid", () => {
  it("lists projects with status badges", async () => {
    const w = mountGrid();
    await flushPromises();
    const cards = w.findAll("[data-testid^='project-']").filter((c) => c.attributes("data-testid") !== "project-grid" && c.attributes("data-testid") !== "project-status");
    expect(cards).toHaveLength(2);
    const badges = w.findAll("[data-testid='project-status']").map((b) => b.text());
    expect(badges).toEqual(["Ready", "Crashed"]);
    w.unmount();
  });

  it("links each row to its project page, but the actions menu does not navigate", async () => {
    const w = mountGrid();
    await flushPromises();
    const row = w.find("[data-testid='project-a']");
    expect(row.element.tagName).toBe("A");
    expect(row.attributes("href")).toBe("/p/a");
    expect(row.text()).toContain("localhost:4000");
    await w.find("[aria-label='Actions for Site a']").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe("/");
    await row.trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe("/p/a");
    w.unmount();
  });

  it("creates a project from the dialog", async () => {
    const w = mountGrid();
    await flushPromises();
    byText("New project")!.click();
    await flushPromises();
    const input = document.body.querySelector<HTMLInputElement>("input[aria-label='Project name']")!;
    input.value = "Landing";
    input.dispatchEvent(new Event("input"));
    await flushPromises();
    document.body.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    await flushPromises();
    expect(api.createProject).toHaveBeenCalledWith("Landing");
    expect(router.currentRoute.value.path).toBe("/p/new");
    w.unmount();
  });

  it("asks for confirmation before deleting, then deletes", async () => {
    const w = mountGrid();
    await flushPromises();
    await chooseDelete(w, "Site a");
    expect(api.deleteProject).not.toHaveBeenCalled();
    expect(document.body.textContent).toContain("Delete Site a?");
    byText("Delete project")!.click();
    await flushPromises();
    expect(api.deleteProject).toHaveBeenCalledWith("a");
    expect(w.emitted("deleted")?.[0]).toEqual(["a"]);
    expect(w.text()).not.toContain("Site a");
    w.unmount();
  });

  it("does not delete when cancelled", async () => {
    const w = mountGrid();
    await flushPromises();
    await chooseDelete(w, "Site b");
    byText("Cancel")!.click();
    await flushPromises();
    expect(api.deleteProject).not.toHaveBeenCalled();
    w.unmount();
  });
});
