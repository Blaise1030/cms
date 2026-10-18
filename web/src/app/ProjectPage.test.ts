// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter, type Router } from "vue-router";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { Project } from "@/core/types";
import { useProjectsStore } from "@/modules/projects/store";
import { useChangesStore } from "@/modules/changes/store";
import { useAgentOutboxStore } from "@/modules/chat/outbox";
import { routes } from "@/router";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon, toastError: vi.fn(), openChat: vi.fn() }));
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
    props: ["showChat"],
    methods: { openChat: () => h.openChat() },
    template:
      "<div data-testid='shell' :data-show-chat='showChat'><slot name='menubar' /><slot name='workspace' /><slot name='chat' /></div>",
  },
  EditorMenubar: true,
  Workspace: { props: ["project"], template: "<div data-testid='workspace'>{{ project.id }}</div>" },
  AgentPanel: {
    props: ["projectId"],
    template: "<div data-testid='chat'>{{ projectId }}</div>",
  },
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

beforeEach(() => {
  h.toastError.mockClear();
  h.openChat.mockClear();
});

describe("ProjectPage", () => {
  it("shows the project named by the route", async () => {
    const w = await mountAt("/p/a");
    expect(w.find("[data-testid='workspace']").text()).toBe("a");
    expect(w.find("[data-testid='chat']").text()).toBe("a");
    expect(router.currentRoute.value.path).toBe("/p/a/preview");
  });

  it("shows the agent beside the design editor", async () => {
    const w = await mountAt("/p/a/design");
    expect(w.find("[data-testid='shell']").attributes("data-show-chat")).toBe("true");
  });

  it("opens the agent when something is sent to it, even from a tab that hides it", async () => {
    const w = await mountAt("/p/a/history");
    const shell = () => w.find("[data-testid='shell']");
    expect(shell().attributes("data-show-chat")).toBe("false");

    useAgentOutboxStore().send({ kind: "text", text: "Restore this version", submit: true });
    useAgentOutboxStore().drain();
    await flushPromises();

    expect(shell().attributes("data-show-chat")).toBe("true");
    expect(h.openChat).toHaveBeenCalled();

    await router.push("/p/a/preview");
    await router.push("/p/a/history");
    await flushPromises();
    expect(shell().attributes("data-show-chat")).toBe("false");
  });

  it("sends an unknown project id home with a toast", async () => {
    await mountAt("/p/gone");
    expect(router.currentRoute.value.path).toBe("/");
    expect(h.toastError).toHaveBeenCalledWith('Project "gone" was not found');
  });

  it("toasts checkpoint.blocked for the open project", async () => {
    await mountAt("/p/a");
    h.daemon.emit({ type: "checkpoint.blocked", projectId: "a", reason: "Start a change first" });
    expect(h.toastError).toHaveBeenCalledWith("Start a change first");
  });

  it("ignores checkpoint.blocked for another project", async () => {
    await mountAt("/p/a");
    h.daemon.emit({ type: "checkpoint.blocked", projectId: "other", reason: "not this one" });
    expect(h.toastError).not.toHaveBeenCalled();
  });

  it("stops PR status polling when the page unmounts", async () => {
    const w = await mountAt("/p/a");
    const stop = vi.spyOn(useChangesStore(), "stopStatusPolling");
    w.unmount();
    expect(stop).toHaveBeenCalled();
  });
});
