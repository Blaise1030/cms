// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { ref, type Ref } from "vue";

/* eslint-disable @typescript-eslint/no-explicit-any */
const h = vi.hoisted(() => ({
  insert: null as any,
  send: null as any,
  busy: null as unknown as Ref<boolean>,
  emitOpenTerminal: null as unknown as () => void,
}));

vi.mock("./ChatPanel.vue", async () => {
  const { defineComponent, h: render } = await import("vue");
  return {
    // Loaded through defineAsyncComponent, which only unwraps `default` from an ES module.
    __esModule: true,
    default: defineComponent({
      props: { projectId: String, active: Boolean },
      emits: ["open-terminal"],
      setup(props, { emit, expose }) {
        h.emitOpenTerminal = () => emit("open-terminal");
        expose({ insert: h.insert, send: h.send, busy: h.busy });
        return () => render("div", { "data-testid": "chat-stub", "data-active": String(props.active) });
      },
    }),
  };
});
vi.mock("@/modules/terminal/TerminalPanel.vue", async () => {
  const { defineComponent, h: render } = await import("vue");
  return {
    default: defineComponent({
      props: { projectId: String, active: Boolean },
      setup(props) {
        return () => render("div", { "data-testid": "terminal-stub", "data-active": String(props.active) });
      },
    }),
  };
});

import AgentPanel from "./AgentPanel.vue";
import { useAgentOutboxStore } from "@/modules/terminal/outbox";

enableAutoUnmount(afterEach);

const element = { tag: "<h1>", selector: "h1", text: "Hi" } as never;
const elementText = '[element <h1> "Hi" at `h1`]';

beforeEach(() => {
  h.insert = vi.fn();
  h.send = vi.fn(async () => undefined);
  h.busy = ref(false);
  setActivePinia(createPinia());
});

async function mountPanel() {
  const w = mount(AgentPanel, { props: { projectId: "p" }, attachTo: document.body });
  await flushPromises();
  return w;
}

const stub = (w: Awaited<ReturnType<typeof mountPanel>>, id: string) => w.find(`[data-testid='${id}']`);

async function showTerminal(w: Awaited<ReturnType<typeof mountPanel>>) {
  const tab = w.find("[data-testid='agent-tab-terminal']");
  await tab.trigger("mousedown", { button: 0 });
  await tab.trigger("click");
  await flushPromises();
}

describe("AgentPanel", () => {
  it("opens on the Chat tab with both panels mounted", async () => {
    const w = await mountPanel();
    expect(w.find("[data-testid='agent-tab-chat']").attributes("data-state")).toBe("active");
    expect(stub(w, "chat-stub").attributes("data-active")).toBe("true");
    expect(stub(w, "terminal-stub").exists()).toBe(true);
    expect(stub(w, "terminal-stub").attributes("data-active")).toBe("false");
    expect(stub(w, "chat-stub").isVisible()).toBe(true);
    expect(stub(w, "terminal-stub").isVisible()).toBe(false);
  });

  it("switches to Terminal when the chat asks for it", async () => {
    const w = await mountPanel();
    h.emitOpenTerminal();
    await flushPromises();
    expect(w.find("[data-testid='agent-tab-terminal']").attributes("data-state")).toBe("active");
    expect(stub(w, "terminal-stub").attributes("data-active")).toBe("true");
    expect(stub(w, "chat-stub").attributes("data-active")).toBe("false");
    expect(stub(w, "terminal-stub").isVisible()).toBe(true);
  });

  it("puts a picked element in the chat input without sending it", async () => {
    await mountPanel();
    useAgentOutboxStore().send({ kind: "element", element });
    await flushPromises();
    expect(h.insert).toHaveBeenCalledWith(elementText);
    expect(h.send).not.toHaveBeenCalled();
    expect(useAgentOutboxStore().messages).toEqual([]);
  });

  it("leaves outbox messages for the terminal while Terminal is active", async () => {
    const w = await mountPanel();
    await showTerminal(w);
    useAgentOutboxStore().send({ kind: "element", element });
    await flushPromises();
    expect(h.insert).not.toHaveBeenCalled();
    expect(useAgentOutboxStore().messages).toHaveLength(1);
  });

  it("sends a request in Chat", async () => {
    await mountPanel();
    useAgentOutboxStore().send({ kind: "text", text: "Sync please", submit: true });
    await flushPromises();
    expect(h.send).toHaveBeenCalledWith("Sync please");
    expect(h.insert).not.toHaveBeenCalled();
    expect(useAgentOutboxStore().messages).toEqual([]);
  });

  it("holds a request while the chat is busy, and sends it once it isn't", async () => {
    h.busy.value = true;
    await mountPanel();
    useAgentOutboxStore().send({ kind: "text", text: "Sync please", submit: true });
    useAgentOutboxStore().send({ kind: "element", element });
    await flushPromises();
    expect(h.send).not.toHaveBeenCalled();
    expect(h.insert).toHaveBeenCalledWith(elementText);
    expect(useAgentOutboxStore().messages).toEqual([{ kind: "text", text: "Sync please", submit: true }]);

    h.busy.value = false;
    await flushPromises();
    expect(h.send).toHaveBeenCalledWith("Sync please");
    expect(useAgentOutboxStore().messages).toEqual([]);
  });

  it("sends one request per turn and keeps the rest until the turn ends", async () => {
    h.send = vi.fn(async () => {
      h.busy.value = true;
    });
    await mountPanel();
    useAgentOutboxStore().send({ kind: "text", text: "First", submit: true });
    useAgentOutboxStore().send({ kind: "text", text: "Second", submit: true });
    await flushPromises();
    expect(h.send).toHaveBeenCalledTimes(1);
    expect(h.send).toHaveBeenCalledWith("First");
    expect(useAgentOutboxStore().messages).toEqual([{ kind: "text", text: "Second", submit: true }]);

    h.busy.value = false;
    await flushPromises();
    expect(h.send).toHaveBeenCalledTimes(2);
    expect(h.send).toHaveBeenLastCalledWith("Second");
    expect(useAgentOutboxStore().messages).toEqual([]);
  });
});
