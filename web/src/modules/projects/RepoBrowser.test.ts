// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter, type Router } from "vue-router";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useGithubStore } from "@/modules/github/store";
import type { GithubStatus, Project } from "@/core/types";
import { useProjectsStore } from "./store";
import RepoBrowser from "./RepoBrowser.vue";

const user = { login: "ada", id: 1, name: "Ada", avatarUrl: "" };

const repos = [
  { fullName: "acme/site", private: true, defaultBranch: "main", canPush: true, pushedAt: "2026-10-20T12:00:00Z" },
  { fullName: "acme/docs", private: false, defaultBranch: "main", canPush: false, pushedAt: null },
];

const project: Project = { id: "site", name: "site", port: 4000, devserver: "starting", autoApprove: false, repo: "acme/site" };

const signedInStatus: GithubStatus = { configured: true, deviceFlow: false, source: "cli", user };
const signedOutStatus: GithubStatus = { configured: false, deviceFlow: false, source: null, user: null };

enableAutoUnmount(afterEach);

let router: Router;
function mountBrowser({ api = {}, signedIn = true }: { api?: Record<string, unknown>; signedIn?: boolean } = {}) {
  h.daemon = createFakeDaemon({
    githubRepos: vi.fn(async () => repos),
    githubStatus: vi.fn(async () => (signedIn ? signedInStatus : signedOutStatus)),
    ...api,
  });
  setActivePinia(createPinia());
  if (signedIn) useGithubStore().user = user;
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
const authLoading = () => document.body.querySelector("[data-testid='repo-auth-loading']");

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

  it("does not offer a retry that would only refetch the list after a clone failure", async () => {
    const cloneProject = vi.fn(async () => {
      throw new Error("clone boom");
    });
    const githubRepos = vi.fn(async () => repos);
    mountBrowser({ api: { cloneProject, githubRepos } });
    await flushPromises();
    expect(githubRepos).toHaveBeenCalledTimes(1);

    firstOpen().click();
    await flushPromises();
    const alert = document.body.querySelector("[role='alert']");
    expect(alert?.textContent).toContain("clone boom");
    expect(alert?.textContent).not.toContain("Retry");
    expect(githubRepos).toHaveBeenCalledTimes(1);
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

  it("names the open control and marks it busy while cloning", async () => {
    let release!: (p: unknown) => void;
    const cloneProject = vi.fn(() => new Promise((r) => (release = r)));
    mountBrowser({ api: { cloneProject } });
    await flushPromises();
    expect(firstOpen().getAttribute("aria-label")).toBe("Open acme/site");

    firstOpen().click();
    await flushPromises();
    expect(firstOpen().getAttribute("aria-label")).toBe("Opening acme/site");
    expect(firstOpen().getAttribute("aria-busy")).toBe("true");

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
    // A failed load must not also claim the account has no repositories.
    expect(document.body.textContent).not.toContain("No repositories yet.");

    [...document.body.querySelectorAll("button")].find((b) => b.textContent?.includes("Retry"))!.click();
    await flushPromises();
    expect(githubRepos).toHaveBeenCalledTimes(2);
    expect(items()).toHaveLength(2);
  });

  it("keeps the load error visible when the rejection signs the user out", async () => {
    const githubRepos = vi.fn(async () => {
      useGithubStore().user = null;
      throw new Error("bad credentials");
    });
    mountBrowser({ api: { githubRepos } });
    await flushPromises();
    expect(document.body.querySelector("[role='alert']")?.textContent).toContain("bad credentials");
  });

  it("does not show the sign-in prompt before the auth status is known", async () => {
    let resolve!: (s: GithubStatus) => void;
    const githubStatus = vi.fn(() => new Promise<GithubStatus>((r) => (resolve = r)));
    mountBrowser({ api: { githubStatus }, signedIn: false });

    expect(authLoading()).not.toBeNull();
    expect(document.body.querySelector("[data-testid='repo-sign-in']")).toBeNull();
    expect(document.body.textContent).not.toContain("gh auth login");

    resolve({ configured: true, deviceFlow: true, source: null, user: null });
    await flushPromises();
    expect(authLoading()).toBeNull();
    expect(document.body.querySelector("[data-testid='repo-sign-in']")).not.toBeNull();
  });

  it("does not request repositories when signed out", async () => {
    const githubRepos = vi.fn(async () => repos);
    mountBrowser({ api: { githubRepos }, signedIn: false });
    await flushPromises();
    expect(document.body.textContent).toContain("gh auth login");
    expect(githubRepos).not.toHaveBeenCalled();
  });

  it("offers sign-in instead of repositories when signed out", async () => {
    mountBrowser({
      signedIn: false,
      api: { githubStatus: vi.fn(async () => ({ ...signedOutStatus, configured: true, deviceFlow: true })) },
    });
    const gh = useGithubStore();
    await flushPromises();
    expect(document.body.querySelector("[data-testid='repo-sign-in']")).not.toBeNull();

    gh.deviceFlow = false;
    await flushPromises();
    expect(document.body.querySelector("[data-testid='repo-sign-in']")).toBeNull();
    expect(document.body.textContent).toContain("gh auth login");
  });

  it("starts sign-in when the signed-out prompt is clicked", async () => {
    mountBrowser({
      signedIn: false,
      api: { githubStatus: vi.fn(async () => ({ ...signedOutStatus, configured: true, deviceFlow: true })) },
    });
    await flushPromises();
    const gh = useGithubStore();
    const signIn = vi.spyOn(gh, "signIn").mockResolvedValue();

    document.body.querySelector<HTMLElement>("[data-testid='repo-sign-in']")!.click();
    expect(signIn).toHaveBeenCalledOnce();
  });

  it("says there are no repositories when the account has none", async () => {
    mountBrowser({ api: { githubRepos: vi.fn(async () => []) } });
    await flushPromises();
    expect(document.body.textContent).toContain("No repositories yet.");
    expect(document.body.textContent).not.toContain("No repositories match.");
  });

  it("says nothing matches when the filter excludes everything", async () => {
    mountBrowser();
    await flushPromises();
    const filter = document.body.querySelector<HTMLInputElement>("[aria-label='Search repositories']")!;
    filter.value = "zzz";
    filter.dispatchEvent(new Event("input"));
    await flushPromises();
    expect(document.body.textContent).toContain("No repositories match.");
    expect(document.body.textContent).not.toContain("No repositories yet.");
  });

  it("shows how long ago each repository was last pushed", async () => {
    const pushedAt = new Date(Date.now() - 3 * 24 * 3600 * 1000).toISOString();
    const githubRepos = vi.fn(async () => [
      { fullName: "acme/recent", private: false, defaultBranch: "main", canPush: true, pushedAt },
      { fullName: "acme/never", private: false, defaultBranch: "main", canPush: true, pushedAt: null },
    ]);
    mountBrowser({ api: { githubRepos } });
    await flushPromises();

    const [recent, never] = items();
    expect(recent.textContent).toContain("3 days ago");
    expect(never.textContent).not.toMatch(/ago|just now/);
  });

  it("shows at most five repositories", async () => {
    const many = Array.from({ length: 8 }, (_, i) => ({
      fullName: `acme/repo-${i + 1}`,
      private: false,
      defaultBranch: "main",
      canPush: true,
      pushedAt: null,
    }));
    mountBrowser({ api: { githubRepos: vi.fn(async () => many) } });
    await flushPromises();
    expect(items()).toHaveLength(5);
    expect(items()[0]!.textContent).toContain("acme/repo-1");
  });
});
