// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter, type Router } from "vue-router";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { Project } from "@/core/types";
import { useProjectsStore } from "@/modules/projects/store";
import { routes } from "@/router";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon, toastError: vi.fn() }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { error: h.toastError } }));

import ProjectPage from "./ProjectPage.vue";

const project = (id: string): Project => ({
  id,
  name: `Site ${id}`,
  port: 4000,
  devserver: "ready",
  autoApprove: false,
  repo: null,
});

// The page's children have their own tests; stubs keep this one about routing.
const stubs = {
  Shell: {
    template:
      "<div><slot name='menubar' /><slot name='workspace' /><slot name='chat' /></div>",
  },
  EditorMenubar: { template: "<button data-testid='back' @click=\"$emit('back')\" />" },
  Workspace: { props: ["project"], template: "<div data-testid='workspace'>{{ project.id }}</div>" },
  ChatPanel: { props: ["projectId"], template: "<div data-testid='chat'>{{ projectId }}</div>" },
  GithubAccount: true,
};

let router: Router;

async function mountAt(path: string) {
  h.daemon = createFakeDaemon({ listProjects: vi.fn(async () => [project("a")]) });
  setActivePinia(createPinia());
  router = createRouter({ history: createMemoryHistory(), routes });
  await router.push(path);
  const id = router.currentRoute.value.params.id as string;
  const w = mount(ProjectPage, { props: { id }, global: { plugins: [router], stubs } });
  await useProjectsStore().load();
  await flushPromises();
  return w;
}

beforeEach(() => h.toastError.mockClear());

describe("ProjectPage", () => {
  it("shows the project named by the route", async () => {
    const w = await mountAt("/p/a");
    expect(w.find("[data-testid='workspace']").text()).toBe("a");
    expect(w.find("[data-testid='chat']").text()).toBe("a");
    expect(router.currentRoute.value.path).toBe("/p/a");
  });

  it("goes back to the project list from the menubar", async () => {
    const w = await mountAt("/p/a");
    await w.find("[data-testid='back']").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe("/");
  });

  it("sends an unknown project id home with a toast", async () => {
    await mountAt("/p/gone");
    expect(router.currentRoute.value.path).toBe("/");
    expect(h.toastError).toHaveBeenCalledWith('Project "gone" was not found');
  });
});
