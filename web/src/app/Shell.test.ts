// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import Shell from "./Shell.vue";

function mountShell(chatDetached: boolean) {
  return mount(Shell, {
    props: { chatDetached },
    slots: { chat: "<div data-testid='chat'>chat</div>" },
    attachTo: document.body,
  });
}

const chatPanel = (w: ReturnType<typeof mountShell>) => w.find("[data-panel-id='chat']");

describe("Shell", () => {
  it("collapses the chat column and hides its toggle while the chat is detached", async () => {
    const w = mountShell(false);
    await flushPromises();
    expect(chatPanel(w).attributes("data-state")).toBe("expanded");

    await w.setProps({ chatDetached: true });
    await flushPromises();
    expect(chatPanel(w).attributes("data-state")).toBe("collapsed");
    expect(w.find("[data-testid='chat-toggle']").exists()).toBe(false);
    // Kept mounted, so a live agent session survives.
    expect(w.find("[data-testid='chat']").exists()).toBe(true);

    await w.setProps({ chatDetached: false });
    await flushPromises();
    expect(chatPanel(w).attributes("data-state")).toBe("expanded");
    expect(w.find("[data-testid='chat-toggle']").exists()).toBe(true);
    w.unmount();
  });
});
