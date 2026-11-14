// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import Shell from "./Shell.vue";

function mountShell(chatDetached: boolean, showChat = true) {
  return mount(Shell, {
    props: { chatDetached, showChat },
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

  it("gives the workspace the chat column's space when chat is hidden", async () => {
    const w = mountShell(false, true);
    await flushPromises();
    const workspace = () => w.find("[data-panel-id='workspace']");
    const size = () => Number(workspace().attributes("data-panel-size"));
    const before = size();
    expect(before).toBeLessThan(100);

    await w.setProps({ showChat: false });
    await flushPromises();
    expect(w.find("[data-testid='chat']").exists()).toBe(false);
    // A lone panel grows to fill the group.
    expect(size()).toBe(1);

    await w.setProps({ showChat: true });
    await flushPromises();
    expect(w.find("[data-testid='chat']").exists()).toBe(true);
    expect(size()).toBeCloseTo(before, 0);
    w.unmount();
  });

  describe("top bar", () => {
    afterEach(() => delete document.documentElement.dataset.desktop);

    const topBar = (w: ReturnType<typeof mountShell>) => w.find("[data-testid='top-bar']");

    it("is a plain header in the browser", () => {
      const w = mountShell(false);
      expect(topBar(w).attributes("data-tauri-drag-region")).toBeUndefined();
      expect(topBar(w).classes()).not.toContain("pl-20");
      w.unmount();
    });

    it("drags the macOS desktop window and clears the traffic lights", async () => {
      document.documentElement.dataset.desktop = "macos";
      const w = mountShell(false);
      await flushPromises();
      expect(topBar(w).attributes("data-tauri-drag-region")).toBe("deep");
      expect(topBar(w).classes()).toContain("pl-20");

      // Its buttons stay clickable.
      await w.find("[data-testid='chat-toggle']").trigger("click");
      await flushPromises();
      expect(chatPanel(w).attributes("data-state")).toBe("collapsed");
      w.unmount();
    });

    it("leaves less room for the traffic lights beside the project rail", () => {
      document.documentElement.dataset.desktop = "macos";
      const w = mount(Shell, { slots: { rail: "<nav class='w-14' />" } });
      expect(topBar(w).classes()).not.toContain("pl-20");
      expect(topBar(w).classes()).toContain("pl-7");
      w.unmount();
    });
  });
});
