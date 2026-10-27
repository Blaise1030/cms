// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { ref, type Ref } from "vue";

/* eslint-disable @typescript-eslint/no-explicit-any */
const h = vi.hoisted(() => ({
  insert: null as any,
  insertReference: null as any,
  send: null as any,
  busy: null as unknown as Ref<boolean>,
}));

vi.mock("./ChatPanel.vue", async () => {
  const { defineComponent, h: render } = await import("vue");
  return {
    // Loaded through defineAsyncComponent, which only unwraps `default` from an ES module.
    __esModule: true,
    default: defineComponent({
      props: { projectId: String, active: Boolean },
      setup(props, { expose }) {
        expose({ insert: h.insert, insertReference: h.insertReference, send: h.send, busy: h.busy });
        return () => render("div", { "data-testid": "chat-stub", "data-active": String(props.active) });
      },
    }),
  };
});

import AgentPanel from "./AgentPanel.vue";
import { useAgentOutboxStore } from "@/modules/chat/outbox";

enableAutoUnmount(afterEach);

const element = { tag: "<h1>", selector: "h1", text: "Hi" } as never;
const elementText = '[element <h1> "Hi" at `h1`]';

beforeEach(() => {
  h.insert = vi.fn();
  h.insertReference = vi.fn();
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

describe("AgentPanel", () => {
  it("renders only the chat, active", async () => {
    const w = await mountPanel();
    expect(stub(w, "chat-stub").attributes("data-active")).toBe("true");
    expect(stub(w, "chat-stub").isVisible()).toBe(true);
    expect(w.find("[role='tablist']").exists()).toBe(false);
    expect(w.find("[data-testid='agent-tab-terminal']").exists()).toBe(false);
  });

  it("puts a picked element in the chat input without sending it", async () => {
    await mountPanel();
    useAgentOutboxStore().send({ kind: "element", element });
    await flushPromises();
    expect(h.insertReference).toHaveBeenCalledWith(elementText);
    expect(h.send).not.toHaveBeenCalled();
    expect(useAgentOutboxStore().messages).toEqual([]);
  });

  it("sends a request in Chat", async () => {
    await mountPanel();
    useAgentOutboxStore().send({ kind: "text", text: "Sync please", submit: true });
    await flushPromises();
    expect(h.send).toHaveBeenCalledWith("Sync please");
    expect(h.insert).not.toHaveBeenCalled();
    expect(useAgentOutboxStore().messages).toEqual([]);
  });

  it("forwards a busy chat request into the visible chat queue", async () => {
    h.busy.value = true;
    await mountPanel();
    useAgentOutboxStore().send({ kind: "text", text: "Sync please", submit: true });
    useAgentOutboxStore().send({ kind: "element", element });
    await flushPromises();
    expect(h.send).toHaveBeenCalledWith("Sync please");
    expect(h.insertReference).toHaveBeenCalledWith(elementText);
    expect(useAgentOutboxStore().messages).toEqual([]);
  });

  it("forwards several requests in order for the chat to queue", async () => {
    h.send = vi.fn(async () => {
      h.busy.value = true;
    });
    await mountPanel();
    useAgentOutboxStore().send({ kind: "text", text: "First", submit: true });
    useAgentOutboxStore().send({ kind: "text", text: "Second", submit: true });
    await flushPromises();
    expect(h.send).toHaveBeenCalledTimes(2);
    expect(h.send.mock.calls).toEqual([["First"], ["Second"]]);
    expect(useAgentOutboxStore().messages).toEqual([]);
  });
});
