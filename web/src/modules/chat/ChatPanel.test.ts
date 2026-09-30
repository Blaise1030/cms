// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import ChatPanel from "./ChatPanel.vue";

let api: Record<string, ReturnType<typeof vi.fn>>;

function mountPanel() {
  api = {
    listAgents: vi.fn(async () => [{ id: "claude", command: [] }]),
    history: vi.fn(async () => []),
    startSession: vi.fn(async () => "s1"),
    prompt: vi.fn(async () => "t1"),
    cancelTurn: vi.fn(async () => ({})),
    resolvePermission: vi.fn(async () => ({})),
  };
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return mount(ChatPanel, { props: { projectId: "p", autoApprove: false }, attachTo: document.body });
}

async function send(w: ReturnType<typeof mountPanel>, text: string) {
  await w.find("textarea").setValue(text);
  await w.find("form").trigger("submit");
  await flushPromises();
}

beforeEach(() => localStorage.clear());

describe("ChatPanel", () => {
  it("sends a message and renders streamed assistant text and tool cards", async () => {
    const w = mountPanel();
    await flushPromises();
    await send(w, "make it blue");
    expect(api.prompt).toHaveBeenCalledWith("p", "s1", "make it blue");
    expect(w.find("[data-testid='user-text']").text()).toBe("make it blue");

    h.daemon.emit({ type: "chat.chunk", sessionId: "s1", turnId: "t1", delta: "Working" });
    h.daemon.emit({ type: "chat.chunk", sessionId: "s1", turnId: "t1", delta: " on it" });
    h.daemon.emit({
      type: "chat.tool", sessionId: "s1", turnId: "t1", toolCallId: "c",
      tool: "edit", state: "running", summary: "edit hero.mdx",
    });
    await flushPromises();
    expect(w.find("[data-testid='assistant-text']").text()).toBe("Working on it");
    const card = w.find("[data-testid='tool-card']");
    expect(card.text()).toContain("edit hero.mdx");
    expect(card.find("[data-testid='tool-state']").text()).toBe("running");
    w.unmount();
  });

  it("shows Stop while streaming and cancels the turn", async () => {
    const w = mountPanel();
    await flushPromises();
    await send(w, "go");
    const stop = w.find("[data-testid='stop']");
    expect(stop.exists()).toBe(true);
    expect(w.find("[data-testid='send']").exists()).toBe(false);
    await stop.trigger("click");
    expect(api.cancelTurn).toHaveBeenCalledWith("t1");
    h.daemon.emit({ type: "chat.done", sessionId: "s1", turnId: "t1", stopReason: "cancelled" });
    await flushPromises();
    expect(w.find("[data-testid='stop']").exists()).toBe(false);
    expect(w.find("[data-testid='send']").exists()).toBe(true);
    w.unmount();
  });

  it("renders a permission prompt with Allow/Deny that call the daemon", async () => {
    const w = mountPanel();
    await flushPromises();
    await send(w, "build it");
    h.daemon.emit({
      type: "permission.request", sessionId: "s1", requestId: "r1", turnId: "t1",
      tool: "bash", command: "npm run build",
    });
    await flushPromises();
    const prompt = w.find("[data-testid='permission-prompt']");
    expect(prompt.text()).toContain("npm run build");
    await prompt.find("[data-testid='permission-allow']").trigger("click");
    await flushPromises();
    expect(api.resolvePermission).toHaveBeenCalledWith("r1", "allow");
    expect(w.find("[data-testid='permission-prompt']").exists()).toBe(false);

    h.daemon.emit({ type: "permission.request", sessionId: "s1", requestId: "r2", tool: "bash", turnId: "t1" });
    await flushPromises();
    await w.find("[data-testid='permission-deny']").trigger("click");
    await flushPromises();
    expect(api.resolvePermission).toHaveBeenLastCalledWith("r2", "deny");
    w.unmount();
  });

  it("shows a session-death banner with a restart action", async () => {
    const w = mountPanel();
    await flushPromises();
    await send(w, "go");
    h.daemon.emit({ type: "chat.error", sessionId: "s1", turnId: null, message: "agent crashed" });
    await flushPromises();
    const banner = w.find("[data-testid='session-error']");
    expect(banner.text()).toContain("agent crashed");
    api.startSession!.mockResolvedValueOnce("s2");
    await banner.find("button").trigger("click");
    await flushPromises();
    expect(api.startSession).toHaveBeenCalledTimes(2);
    expect(w.find("[data-testid='session-error']").exists()).toBe(false);
    w.unmount();
  });

  it("sends on Enter but not Shift+Enter", async () => {
    const w = mountPanel();
    await flushPromises();
    const ta = w.find("textarea");
    await ta.setValue("hello");
    await ta.trigger("keydown", { key: "Enter", shiftKey: true });
    await flushPromises();
    expect(api.prompt).not.toHaveBeenCalled();
    await ta.trigger("keydown", { key: "Enter" });
    await flushPromises();
    expect(api.prompt).toHaveBeenCalledWith("p", "s1", "hello");
    w.unmount();
  });

  it("emits toggleAutoApprove", async () => {
    const w = mountPanel();
    await w.find("[data-testid='auto-approve']").trigger("click");
    expect(w.emitted("toggleAutoApprove")).toHaveLength(1);
    w.unmount();
  });
});
