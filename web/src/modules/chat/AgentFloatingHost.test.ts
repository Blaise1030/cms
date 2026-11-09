// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { nextTick } from "vue";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

vi.mock("./AgentPanel.vue", () => ({
  default: {
    name: "AgentPanel",
    emits: ["minimize", "close"],
    template: `
      <div>
        <button type="button" data-testid="agent-minimize" @click="$emit('minimize')">Min</button>
        <button type="button" data-testid="agent-close" @click="$emit('close')">Close</button>
        <div data-testid="agent-panel-stub">chat</div>
      </div>
    `,
  },
}));

import AgentFloatingHost from "./AgentFloatingHost.vue";
import { useChatStore } from "./store";

enableAutoUnmount(afterEach);

beforeEach(() => {
  h.daemon = createFakeDaemon({
    listChats: async () => ({ chats: [], running: null }),
  });
  setActivePinia(createPinia());
});

function chat(id: string, title: string, running = false) {
  return {
    id,
    provider: "cursor" as const,
    kind: "chat" as const,
    title,
    updatedAt: 1,
    live: true,
    running,
  };
}

function mountHost() {
  return mount(AgentFloatingHost, {
    props: { projectId: "p1" },
    attachTo: document.body,
  });
}

describe("AgentFloatingHost", () => {
  it("lists every available session in the bottom bar with running status", async () => {
    const listed = [chat("c1", "SEO audit"), chat("c2", "Homepage", true)];
    h.daemon = createFakeDaemon({
      listChats: async () => ({ chats: listed, running: "c2" }),
    });
    setActivePinia(createPinia());
    const store = useChatStore();

    const w = mountHost();
    await flushPromises();
    store.setChatRunning("p1", "c2", true);
    await nextTick();

    const tabs = w.findAll("[data-testid='agent-session-tab']");
    expect(tabs).toHaveLength(2);
    expect(tabs[0]!.text()).toContain("SEO audit");
    expect(tabs[1]!.text()).toContain("Homepage");
    expect(tabs[1]!.find("[data-testid='agent-tab-status']").attributes("data-state")).toBe("running");
    expect(tabs[0]!.find("[data-testid='agent-tab-status']").attributes("data-state")).toBe("idle");
  });

  it("opens the floating panel from a session tab or +", async () => {
    h.daemon = createFakeDaemon({
      listChats: async () => ({ chats: [chat("c1", "SEO audit")], running: null }),
    });
    setActivePinia(createPinia());
    const store = useChatStore();

    const w = mountHost();
    await flushPromises();
    expect(w.find("[data-testid='agent-floating-panel']").exists()).toBe(false);

    await w.get("[data-testid='agent-session-tab']").trigger("click");
    await flushPromises();
    expect(store.activeChatId("p1")).toBe("c1");
    expect(w.find("[data-testid='agent-floating-panel']").isVisible()).toBe(true);

    await w.get("[data-testid='agent-minimize']").trigger("click");
    await nextTick();
    expect(w.find("[data-testid='agent-floating-panel']").isVisible()).toBe(false);

    await w.get("[data-testid='agent-new-tab']").trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='agent-floating-panel']").isVisible()).toBe(true);
    expect(w.findAll("[data-testid='agent-session-tab']").length).toBeGreaterThanOrEqual(2);
  });

  it("keeps the panel mounted while minimized after opening", async () => {
    h.daemon = createFakeDaemon({
      listChats: async () => ({ chats: [chat("c1", "SEO audit")], running: null }),
    });
    setActivePinia(createPinia());
    const store = useChatStore();

    const w = mountHost();
    await flushPromises();
    store.setActive("p1", "c1");
    w.vm.expand();
    await flushPromises();
    await w.get("[data-testid='agent-minimize']").trigger("click");
    await nextTick();
    expect(w.find("[data-testid='agent-panel-stub']").exists()).toBe(true);
    expect(w.find("[data-testid='agent-floating-panel']").isVisible()).toBe(false);
  });

  it("collapses on panel close without removing sessions from the bar", async () => {
    h.daemon = createFakeDaemon({
      listChats: async () => ({ chats: [chat("c1", "SEO audit")], running: null }),
    });
    setActivePinia(createPinia());
    const store = useChatStore();

    const w = mountHost();
    await flushPromises();
    store.setActive("p1", "c1");
    w.vm.expand();
    await nextTick();
    await w.get("[data-testid='agent-close']").trigger("click");
    await nextTick();
    expect(w.find("[data-testid='agent-floating-panel']").isVisible()).toBe(false);
    expect(w.findAll("[data-testid='agent-session-tab']")).toHaveLength(1);
  });

  it("exposes expand for outbox-driven opens", async () => {
    const w = mountHost();
    w.vm.expand();
    await nextTick();
    expect(w.find("[data-testid='agent-session-tab']").exists()).toBe(true);
    expect(w.find("[data-testid='agent-floating-panel']").isVisible()).toBe(true);
  });

  it("deletes a session from the bar after confirm", async () => {
    const listed = [chat("c1", "SEO audit"), chat("c2", "Homepage")];
    const deleteChat = vi.fn(async () => ({ deleted: true }));
    h.daemon = createFakeDaemon({
      listChats: async () => ({ chats: listed, running: null }),
      deleteChat,
    });
    setActivePinia(createPinia());
    const store = useChatStore();

    const w = mountHost();
    await flushPromises();
    store.setActive("p1", "c1");
    await nextTick();
    expect(w.findAll("[data-testid='agent-session-tab']")).toHaveLength(2);

    // Delete the inactive Homepage tab (deleting the active one would open a fresh draft).
    const homepage = w.findAll("[data-testid='agent-session-tab']")[1]!;
    await homepage.get("[data-testid='agent-tab-delete']").trigger("click");
    await flushPromises();
    document.body.querySelector<HTMLButtonElement>("[data-testid='agent-tab-delete-confirm']")!.click();
    await flushPromises();

    expect(deleteChat).toHaveBeenCalledWith("p1", "c2");
    expect(w.findAll("[data-testid='agent-session-tab']")).toHaveLength(1);
    expect(w.get("[data-testid='agent-session-tab']").text()).toContain("SEO audit");
  });

  it("reorders sessions when a pill is dropped on another", async () => {
    localStorage.clear();
    const listed = [chat("c1", "SEO audit"), chat("c2", "Homepage")];
    h.daemon = createFakeDaemon({
      listChats: async () => ({ chats: listed, running: null }),
    });
    setActivePinia(createPinia());

    const w = mountHost();
    await flushPromises();
    const tabs = w.findAll("[data-testid='agent-session-tab']");
    expect(tabs[0]!.text()).toContain("SEO audit");

    await tabs[1]!.trigger("dragstart", {
      dataTransfer: { setData: vi.fn(), effectAllowed: "move" },
    });
    await tabs[0]!.trigger("drop", {
      dataTransfer: { getData: () => "c2", dropEffect: "move" },
    });
    await nextTick();

    const reordered = w.findAll("[data-testid='agent-session-tab']");
    expect(reordered[0]!.text()).toContain("Homepage");
    expect(reordered[1]!.text()).toContain("SEO audit");
    expect(localStorage.getItem("orso.chat.sessionOrder.p1")).toBe(JSON.stringify(["c2", "c1"]));
  });
});
