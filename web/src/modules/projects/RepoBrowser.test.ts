// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter, type Router } from "vue-router";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useGithubStore } from "@/modules/github/store";
import type { Project } from "@/core/types";
import { useProjectsStore } from "./store";
import RepoBrowser from "./RepoBrowser.vue";

const repos = [
  { fullName: "acme/site", private: true, defaultBranch: "main", canPush: true, pushedAt: "2026-10-20T12:00:00Z" },
  { fullName: "acme/docs", private: false, defaultBranch: "main", canPush: false, pushedAt: null },
];

const project: Project = { id: "site", name: "site", port: 4000, devserver: "starting", autoApprove: false, repo: "acme/site" };

enableAutoUnmount(afterEach);

let router: Router;
function mountBrowser({ api = {}, signedIn = true }: { api?: Record<string, unknown>; signedIn?: boolean } = {}) {
  h.daemon = createFakeDaemon({ githubRepos: vi.fn(async () => repos), ...api });
  setActivePinia(createPinia());
  if (signedIn) useGithubStore().user = { login: "ada", id: 1, name: "Ada", avatarUrl: "" };
  router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: { template: "<div />" } },
      { path: "/p/:id/:tab?", component: { template: "<div />" } },
    ],
  });
  return mount(RepoBrowser, { global: { plugins: [router] }, attachTo: document.body });
}

const items = () => [...document.body.querySelectorAll<HTMLElement>("[data-testid='repo-item']")];
const firstOpen = () => document.body.querySelector<HTMLButtonElement>("[data-testid='repo-item'] button")!;

describe("RepoBrowser", () => {
  it("lists, filters and clones a repository", async () => {
    const cloneProject = vi.fn(async () => project);
    mountBrowser({ api: { cloneProject } });
    await flushPromises();
    expect(items().map((i) => i.textContent)).toEqual([
      expect.stringContaining("acme/site"),
      expect.stringContaining("acme/docs"),
    ]);

    const filter = document.body.querySelector<HTMLInputElement>("[aria-label='Search repositories']")!;
    filter.value = "site";
    filter.dispatchEvent(new Event("input"));
    await flushPromises();
    expect(items()).toHaveLength(1);

    firstOpen().click();
    await flushPromises();
    expect(cloneProject).toHaveBeenCalledWith("acme/site");
    expect(router.currentRoute.value.path).toBe("/p/site/preview");
  });

  it("opens an already-cloned repository without cloning it again", async () => {
    const cloneProject = vi.fn();
    mountBrowser({ api: { cloneProject } });
    await flushPromises();
    useProjectsStore().projects = [{ ...project, id: "local", port: 4010 }];

    firstOpen().click();
    await flushPromises();
    expect(cloneProject).not.toHaveBeenCalled();
    expect(router.currentRoute.value.path).toBe("/p/local/preview");
  });

  it("shows the clone error and stays put", async () => {
    const cloneProject = vi.fn(async () => {
      throw new Error("clone boom");
    });
    mountBrowser({ api: { cloneProject } });
    await flushPromises();

    firstOpen().click();
    await flushPromises();
    expect(document.body.querySelector("[role='alert']")?.textContent).toContain("clone boom");
    expect(router.currentRoute.value.path).toBe("/");
  });

  it("cannot start a second clone while one is in flight", async () => {
    let release!: (p: unknown) => void;
    const cloneProject = vi.fn(() => new Promise((r) => (release = r)));
    mountBrowser({ api: { cloneProject } });
    await flushPromises();

    firstOpen().click();
    await flushPromises();
    firstOpen().click();
    await flushPromises();
    expect(cloneProject).toHaveBeenCalledTimes(1);

    release(project);
    await flushPromises();
  });

  it("shows a load error and retries", async () => {
    const githubRepos = vi
      .fn()
      .mockRejectedValueOnce(new Error("rate limited"))
      .mockResolvedValueOnce(repos);
    mountBrowser({ api: { githubRepos } });
    await flushPromises();
    expect(document.body.querySelector("[role='alert']")?.textContent).toContain("rate limited");

    [...document.body.querySelectorAll("button")].find((b) => b.textContent?.includes("Retry"))!.click();
    await flushPromises();
    expect(githubRepos).toHaveBeenCalledTimes(2);
    expect(items()).toHaveLength(2);
  });

  it("offers sign-in instead of repositories when signed out", async () => {
    mountBrowser({ signedIn: false });
    const gh = useGithubStore();
    gh.deviceFlow = true;
    await flushPromises();
    expect(document.body.querySelector("[data-testid='repo-sign-in']")).not.toBeNull();

    gh.deviceFlow = false;
    await flushPromises();
    expect(document.body.querySelector("[data-testid='repo-sign-in']")).toBeNull();
    expect(document.body.textContent).toContain("gh auth login");
  });
});
