// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { ApiError } from "@/core/api";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import ChatPanel from "./ChatPanel.vue";
import ComposerEditor from "./ComposerEditor.vue";
import { usePreviewStore } from "@/modules/preview/store";

let api: Record<string, ReturnType<typeof vi.fn>>;

function mountPanel() {
  api = {
    listAgents: vi.fn(async () => [{ id: "claude", command: [] }]),
    history: vi.fn(async () => []),
    startSession: vi.fn(async () => "s1"),
    prompt: vi.fn(async () => "t1"),
    cancelTurn: vi.fn(async () => ({})),
    resolvePermission: vi.fn(async () => ({})),
    createChange: vi.fn(async () => ({ branch: "content/launch-week", title: "Launch week" })),
    activeChange: vi.fn(async () => {
      throw new ApiError("ENOENT", "no active change", 404);
    }),
    tree: vi.fn(async () => ({ entries: [] })),
  };
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return mount(ChatPanel, { props: { projectId: "p", autoApprove: false }, attachTo: document.body });
}

async function type(w: ReturnType<typeof mountPanel>, text: string) {
  w.findComponent(ComposerEditor).vm.editor!.commands.insertContent(text);
  await flushPromises();
}

async function send(w: ReturnType<typeof mountPanel>, text: string) {
  await type(w, text);
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

  it("turns an element picked in the preview into a composer chip sent as a reference", async () => {
    const w = mountPanel();
    await flushPromises();
    await type(w, "make ");
    usePreviewStore().pick({
      label: "h1.title",
      tag: '<h1 class="title">',
      selector: "body > h1",
      text: "Hello",
      file: "src/pages/index.astro",
      loc: "4:5",
    });
    await flushPromises();
    expect(w.find("[data-element-ref]").text()).toBe("<h1.title>");
    expect(usePreviewStore().picked).toBeNull();

    await send(w, "bigger");
    expect(api.prompt).toHaveBeenCalledWith(
      "p",
      "s1",
      'make [element <h1 class="title"> "Hello" in src/pages/index.astro:4:5] bigger',
    );
    const bubble = w.find("[data-testid='user-text']");
    expect(bubble.find("[data-testid='user-element-ref']").text()).toBe("<h1.title>");
    expect(bubble.text()).toBe("make <h1.title> bigger");
    w.unmount();
  });

  it("renders assistant markdown", async () => {
    const w = mountPanel();
    await flushPromises();
    await send(w, "summarise");
    h.daemon.emit({
      type: "chat.chunk", sessionId: "s1", turnId: "t1",
      delta: "Done.\n\n- Added `/pricing/`\n- Updated the **header**",
    });
    await flushPromises();
    const text = w.find("[data-testid='assistant-text']");
    expect(text.findAll("li")).toHaveLength(2);
    expect(text.find("code").text()).toBe("/pricing/");
    expect(text.find("strong").text()).toBe("header");
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
    await type(w, "hello");
    const editor = w.find("[data-testid='composer-editor']");
    await editor.trigger("keydown", { key: "Enter", shiftKey: true });
    await flushPromises();
    expect(api.prompt).not.toHaveBeenCalled();
    await editor.trigger("keydown", { key: "Enter" });
    await flushPromises();
    expect(api.prompt).toHaveBeenCalledWith("p", "s1", "hello");
    expect(w.findComponent(ComposerEditor).vm.editor!.isEmpty).toBe(true);
    w.unmount();
  });

  it("emits toggleAutoApprove when the permission mode changes", async () => {
    const w = mountPanel();
    await flushPromises();
    await w.find("[data-testid='auto-approve']").trigger("keydown", { key: "Enter" });
    await flushPromises();
    // Already "ask" (autoApprove: false): picking it again is a no-op.
    document.querySelector<HTMLElement>("[data-testid='mode-ask']")!.click();
    await flushPromises();
    expect(w.emitted("toggleAutoApprove")).toBeUndefined();
    await w.find("[data-testid='auto-approve']").trigger("keydown", { key: "Enter" });
    await flushPromises();
    document.querySelector<HTMLElement>("[data-testid='mode-auto']")!.click();
    await flushPromises();
    expect(w.emitted("toggleAutoApprove")).toHaveLength(1);
    w.unmount();
  });

  it("shows Start a change first on CHANGE_REQUIRED and retries after create", async () => {
    const w = mountPanel();
    await flushPromises();
    api.prompt!
      .mockRejectedValueOnce(new ApiError("CHANGE_REQUIRED", "Start a change first", 409))
      .mockResolvedValueOnce("t2");

    await send(w, "edit the notes");
    expect(api.prompt).toHaveBeenCalledTimes(1);
    expect(w.find("[data-testid='user-text']").exists()).toBe(false);
    expect(document.body.textContent).toContain("Start a change first");

    const input = document.querySelector<HTMLInputElement>("[aria-label='Change title']")!;
    expect(input).toBeTruthy();
    // Native input + Vue v-model: set value then dispatch input for the binding.
    input.value = "New post: Launch week";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await flushPromises();
    const dialogForm = document.querySelector<HTMLFormElement>("[role='dialog'] form")!;
    dialogForm.requestSubmit();
    await flushPromises();

    expect(api.createChange).toHaveBeenCalledWith("p", "New post: Launch week");
    expect(api.prompt).toHaveBeenCalledTimes(2);
    expect(api.prompt).toHaveBeenLastCalledWith("p", "s1", "edit the notes");
    expect(w.find("[data-testid='user-text']").text()).toBe("edit the notes");
    w.unmount();
  });
});
