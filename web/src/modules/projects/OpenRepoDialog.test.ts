// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useGithubStore } from "@/modules/github/store";
import OpenRepoDialog from "./OpenRepoDialog.vue";

const repos = [
  { fullName: "acme/site", private: true, defaultBranch: "main", canPush: true },
  { fullName: "acme/docs", private: false, defaultBranch: "main", canPush: false },
];

describe("OpenRepoDialog", () => {
  it("lists, filters and opens a repository", async () => {
    const cloneProject = vi.fn(async () => ({
      id: "site", name: "site", port: 4000, devserver: "starting", autoApprove: false, repo: "acme/site",
    }));
    h.daemon = createFakeDaemon({ githubRepos: vi.fn(async () => repos), cloneProject });
    setActivePinia(createPinia());
    useGithubStore().user = { login: "ada", id: 1, name: "Ada", avatarUrl: "" };

    const w = mount(OpenRepoDialog, { props: { open: false }, attachTo: document.body });
    await w.setProps({ open: true });
    await flushPromises();
    const items = () => [...document.body.querySelectorAll<HTMLElement>("[data-testid='repo-item']")];
    expect(items().map((i) => i.textContent)).toEqual([
      expect.stringContaining("acme/site"),
      expect.stringContaining("acme/docs"),
    ]);

    const filter = document.body.querySelector<HTMLInputElement>("[aria-label='Filter repositories']")!;
    filter.value = "site";
    filter.dispatchEvent(new Event("input"));
    await flushPromises();
    expect(items()).toHaveLength(1);

    items()[0]!.click();
    await flushPromises();
    expect(cloneProject).toHaveBeenCalledWith("acme/site");
    expect(w.emitted("update:open")?.at(-1)).toEqual([false]);
    w.unmount();
  });
});
