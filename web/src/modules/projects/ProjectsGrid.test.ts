// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter, type Router } from "vue-router";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { GithubUser, Project } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

import { useGithubStore } from "@/modules/github/store";
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

function mountGrid({ user = null }: { user?: GithubUser | null } = {}) {
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  if (user) useGithubStore().user = user;
  router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: { template: "<div />" } },
      { path: "/p/:id/:tab?", component: { template: "<div />" } },
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
  it("puts the account control in the page header", async () => {
    const w = mountGrid({ user: { login: "ada", id: 1, name: "Ada", avatarUrl: "" } });
    await flushPromises();
    const header = w.get("[data-testid='page-header']");
    expect(header.find("[data-testid='account-slot'] [data-testid='github-account']").exists()).toBe(true);
    expect(header.find("svg.paw-mark").exists()).toBe(false);
    w.unmount();
  });

  it("shows recent projects first, then the two ways to start one", async () => {
    const w = mountGrid();
    await flushPromises();
    expect(w.findAll("h2").map((h) => h.text())).toEqual([
      "Recent",
      "Open a project",
      "Create from a template",
    ]);
    w.unmount();
  });

  it("lists projects newest first with status badges", async () => {
    const w = mountGrid();
    await flushPromises();
    const cards = w.findAll("[data-testid^='project-']").filter((c) => c.attributes("data-testid") !== "project-grid" && c.attributes("data-testid") !== "project-status");
    expect(cards).toHaveLength(2);
    expect(cards.map((c) => c.attributes("data-testid"))).toEqual(["project-b", "project-a"]);
    const badges = w.findAll("[data-testid='project-status']").map((b) => b.text());
    expect(badges).toEqual(["Crashed", "Ready"]);
    w.unmount();
  });

  it("shows only the five most recent projects", async () => {
    api.listProjects = vi.fn(async () => ["1", "2", "3", "4", "5", "6", "7"].map((id) => p(id, "ready")));
    const w = mountGrid();
    await flushPromises();
    const rows = w
      .findAll("[data-testid^='project-']")
      .map((c) => c.attributes("data-testid")!)
      .filter((id) => id !== "project-grid" && id !== "project-status");
    expect(rows).toEqual(["project-7", "project-6", "project-5", "project-4", "project-3"]);
    w.unmount();
  });

  it("opens a project from its row, but the actions menu does not navigate", async () => {
    const w = mountGrid();
    await flushPromises();
    const row = w.find("[data-testid='project-a']");
    expect(row.text()).toContain("Site a");
    const open = row.get("a");
    expect(open.attributes("href")).toBe("/p/a/preview");
    expect(open.text()).toBe("Open");
    await w.find("[aria-label='Actions for Site a']").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe("/");
    await open.trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe("/p/a/preview");
    w.unmount();
  });

  it("creates a project from the dialog", async () => {
    const w = mountGrid();
    await flushPromises();
    await w.get("[data-testid='template-astro-marketing']").trigger("click");
    await flushPromises();
    const input = document.body.querySelector<HTMLInputElement>("input[aria-label='Project name']")!;
    input.value = "Landing";
    input.dispatchEvent(new Event("input"));
    await flushPromises();
    document.body.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    await flushPromises();
    expect(api.createProject).toHaveBeenCalledWith("Landing");
    expect(router.currentRoute.value.path).toBe("/p/new/preview");
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

  it("keeps the header draggable only on desktop", async () => {
    const browser = mountGrid();
    await flushPromises();
    expect(browser.get("[data-testid='page-header']").attributes("data-tauri-drag-region")).toBeUndefined();
    browser.unmount();

    document.documentElement.dataset.desktop = "macos";
    try {
      const desktop = mountGrid();
      await flushPromises();
      const header = desktop.get("[data-testid='page-header']");
      expect(header.attributes("data-tauri-drag-region")).toBe("");
      expect(header.element.nextElementSibling).toBe(desktop.get("[data-testid='projects-scroll']").element);
      desktop.unmount();
    } finally {
      delete document.documentElement.dataset.desktop;
    }
  });
});
