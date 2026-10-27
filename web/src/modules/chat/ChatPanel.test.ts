// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createApi } from "@/core/api";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

import ChatPanel from "./ChatPanel.vue";
import { useChatStore } from "./store";

enableAutoUnmount(afterEach);

const ORIGIN = "http://127.0.0.1:7800";

type Call = { method: string; url: string; init: RequestInit };
type Chunk = Record<string, unknown>;
type WireChat = {
  id: string;
  provider: string;
  model: string | null;
  title: string | null;
  updatedAt: number;
  live: boolean;
  running: boolean;
};
type History = { chunks: Chunk[]; provider: string; model: string | null; running: boolean; pendingPermissions?: string[] };

let calls: Call[];
let chats: WireChat[];
let histories: Record<string, History>;
let seq: number;
/** When set, chat history responses wait for it. */
let historyGate: Promise<void> | null;
/** When set, the next message POST fails with it. */
let messageFailure: Response | null;
let messageGate: Promise<void> | null;
let createGate: Promise<void> | null;

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

function finish(id: string, chunks: Chunk[]) {
  histories[id]!.chunks.push(...chunks);
  histories[id]!.running = false;
  const chat = chats.find((c) => c.id === id);
  if (chat) chat.running = false;
  h.daemon.emit({ type: "chat.turn", projectId: "p1", chatId: id });
}

async function fetchMock(input: RequestInfo | URL, init: RequestInit = {}): Promise<Response> {
  const url = String(input);
  const method = (init.method ?? "GET").toUpperCase();
  calls.push({ method, url, init });
  const path = url.replace(ORIGIN, "");
  if (path === "/api/chat/providers")
    return json({
      providers: ["cursor", "claude", "codex", "opencode", "shell"].map((id) => ({
        id,
        command: [id],
        acp: [id],
      })),
    });
  const models = path.match(/^\/api\/chat\/providers\/([^/]+)\/models$/);
  if (models)
    return json({
      models: [
        { id: `${models[1]}-fast`, name: "Fast" },
        { id: `${models[1]}-smart`, name: "Smart" },
      ],
    });
  if (path === "/api/chat/p1/attachments" && method === "POST")
    return json({ path: "/state/p1/attachments/abc123.png" });
  if (path === "/api/chat/p1" && method === "GET")
    return json({ chats, running: chats.find((c) => c.running)?.id ?? null });
  if (path === "/api/chat/p1" && method === "POST") {
    if (createGate) {
      const gate = createGate;
      createGate = null;
      await gate;
    }
    const body = JSON.parse(String(init.body));
    const created: WireChat = {
      id: `c${++seq}`,
      provider: body.provider,
      model: body.model ?? null,
      title: null,
      updatedAt: Date.now(),
      live: true,
      running: false,
    };
    chats.unshift(created);
    histories[created.id] = {
      chunks: [],
      provider: created.provider,
      model: created.model,
      running: false,
    };
    return json(created, 201);
  }
  const stop = path.match(/^\/api\/chat\/p1\/([^/]+)\/stop$/);
  if (stop && method === "POST") {
    const id = stop[1]!;
    const chat = chats.find((c) => c.id === id);
    if (chat) chat.running = false;
    if (histories[id]) histories[id]!.running = false;
    return json({ stopped: true });
  }
  const model = path.match(/^\/api\/chat\/p1\/([^/]+)\/model$/);
  if (model && method === "POST") {
    const id = model[1]!;
    const body = JSON.parse(String(init.body));
    const chat = chats.find((c) => c.id === id);
    if (chat) chat.model = body.modelId;
    if (histories[id]) histories[id]!.model = body.modelId;
    return json({ model: body.modelId });
  }
  if (/^\/api\/chat\/p1\/[^/]+\/approve$/.test(path) && method === "POST") return new Response(null, { status: 204 });
  const message = path.match(/^\/api\/chat\/p1\/([^/]+)\/message$/);
  if (message && method === "POST") {
    if (messageGate) {
      const gate = messageGate;
      messageGate = null;
      await gate;
    }
    if (messageFailure) return messageFailure;
    const id = message[1]!;
    const body = JSON.parse(String(init.body));
    histories[id]!.chunks.push({ type: "user_text", session_id: "s", text: body.text });
    histories[id]!.running = true;
    const chat = chats.find((c) => c.id === id);
    if (chat) chat.running = true;
    return json({ accepted: true }, 202);
  }
  const history = path.match(/^\/api\/chat\/p1\/([^/]+)$/);
  if (history && method === "GET") {
    if (historyGate) await historyGate;
    const found = histories[decodeURIComponent(history[1]!)];
    return found ? json(found) : json({ error: { code: "NOT_FOUND", message: "no such chat" } }, 404);
  }
  if (history && method === "DELETE") {
    const id = decodeURIComponent(history[1]!);
    chats = chats.filter((c) => c.id !== id);
    delete histories[id];
    return json({ deleted: true });
  }
  return json({ error: { code: "UNKNOWN", message: `unrouted ${method} ${path}` } }, 500);
}

const messagePosts = () => calls.filter((c) => c.method === "POST" && c.url.endsWith("/message"));
const createPosts = () =>
  calls.filter((c) => c.method === "POST" && c.url === `${ORIGIN}/api/chat/p1`);
const textOf = (call: Call) => JSON.parse(String(call.init.body)).text as string;

beforeEach(() => {
  localStorage.clear();
  calls = [];
  chats = [];
  histories = {};
  seq = 0;
  historyGate = null;
  messageFailure = null;
  messageGate = null;
  createGate = null;
  vi.stubGlobal("fetch", vi.fn(fetchMock));
  const api = createApi({ origin: ORIGIN, token: "tok" }, (i, init) => fetchMock(i, init));
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
});

afterEach(() => {
  vi.unstubAllGlobals();
});

async function mountPanel() {
  const w = mount(ChatPanel, { props: { projectId: "p1", active: true }, attachTo: document.body });
  await flushPromises();
  return w;
}

async function send(w: Awaited<ReturnType<typeof mountPanel>>, text: string) {
  await w.find("[data-testid='chat-input']").setValue(text);
  await w.find("[data-testid='chat-send']").trigger("click");
  await vi.waitFor(() => expect(messagePosts().length).toBeGreaterThan(0));
  await flushPromises();
}

/** The agent and model picker: a portalled popover with a searchable list grouped by agent. */
function agentPicker(w: Awaited<ReturnType<typeof mountPanel>>) {
  const trigger = () => w.find("[data-testid='chat-agent']");
  const list = () => [...document.body.querySelectorAll<HTMLElement>("[data-testid='chat-model-item']")];
  return {
    label: () => trigger().text(),
    provider: () => trigger().attributes("data-provider"),
    groups: () =>
      [...document.body.querySelectorAll<HTMLElement>("[data-testid='chat-agent-group']")].map((g) => g.dataset.provider),
    open: async () => {
      await trigger().trigger("click");
      await flushPromises();
    },
    items: () => list().map((i) => i.dataset.value),
    search: async (text: string) => {
      const input = document.body.querySelector<HTMLInputElement>("[data-testid='chat-agent-search']")!;
      input.value = text;
      input.dispatchEvent(new Event("input"));
      await flushPromises();
    },
    choose: async (id: string) => {
      list()
        .find((i) => i.dataset.value === id)!
        .click();
      await flushPromises();
    },
  };
}

describe("ChatPanel", () => {
  it("shows an approval card for a pending permission and posts the answer", async () => {
    chats = [{ id: "c1", provider: "cursor", model: null, title: "Chat", updatedAt: 1, live: true, running: true }];
    histories.c1 = {
      provider: "cursor", model: null, running: true,
      chunks: [
        { type: "user_text", session_id: "s", text: "go" },
        { type: "permission", session_id: "s", request_id: "7", tool_call: { title: "Run npm install" }, options: [] },
      ],
      pendingPermissions: ["7"],
    } as History;
    useChatStore().setActive("p1", "c1");
    const w = await mountPanel();
    expect(w.find("[data-testid='chat-approval-title']").text()).toBe("Run npm install");
    await w.find("[data-testid='chat-approval-allow']").trigger("click");
    await flushPromises();
    const post = calls.find((c) => c.url.endsWith("/api/chat/p1/c1/approve"))!;
    expect(JSON.parse(String(post.init.body))).toEqual({ requestId: "7", allow: true });
    expect(w.find("[data-testid='chat-approval']").exists()).toBe(false);
  });

  it("offers an auto-approve switch", async () => {
    const w = await mountPanel();
    expect(w.find("[data-testid='chat-auto-approve']").exists()).toBe(true);
  });

  it("starts with recent messages and loads earlier history when scrolling to the top", async () => {
    chats = [{ id: "c1", provider: "cursor", model: null, title: "Long chat", updatedAt: 1, live: true, running: false }];
    histories.c1 = {
      provider: "cursor", model: null, running: false,
      chunks: Array.from({ length: 12 }, (_, i) => [
        { type: "user_text", session_id: "s", text: `Prompt ${i}` },
        { type: "text_delta", session_id: "s", text: `Reply ${i}` },
        { type: "turn_end", session_id: "s", stop_reason: "end_turn" },
      ]).flat(),
    };
    useChatStore().setActive("p1", "c1");
    const w = await mountPanel();
    expect(w.findAll("[data-slot='message-scroller-item']")).toHaveLength(10);
    expect(w.find("[data-testid='chat-load-earlier']").exists()).toBe(false);
    expect(w.text()).toContain("Reply 11");
    expect(w.text()).not.toContain("Prompt 0");
    const viewport = w.get<HTMLElement>("[data-testid='chat-viewport']");
    viewport.element.scrollTop = 200;
    await viewport.trigger("scroll");
    expect(w.findAll("[data-slot='message-scroller-item']")).toHaveLength(10);
    viewport.element.scrollTop = 60;
    await viewport.trigger("scroll");
    expect(w.findAll("[data-slot='message-scroller-item']")).toHaveLength(20);
    viewport.element.scrollTop = 0;
    await viewport.trigger("scroll");
    expect(w.findAll("[data-slot='message-scroller-item']")).toHaveLength(24);
    expect(w.text()).toContain("Prompt 0");
  });

  it("creates a chat, posts only the new message with auth, then renders the saved reply", async () => {
    const w = await mountPanel();
    await send(w, "hi");
    const create = createPosts()[0]!;
    expect(JSON.parse(String(create.init.body))).toEqual({ provider: "cursor" });
    const message = messagePosts()[0]!;
    expect(textOf(message)).toBe("hi");
    expect(new Headers(message.init.headers).get("Authorization")).toBe("Bearer tok");
    expect(w.text()).toContain("hi");

    const id = useChatStore().activeChatId("p1")!;
    finish(id, [
      { type: "thought_delta", session_id: "s", text: "Looking at about.md" },
      {
        type: "tool_call",
        session_id: "s",
        tool_call: {
          toolCallId: "t1",
          kind: "edit",
          title: "Edit about.md",
          status: "pending",
          locations: [{ path: "about.md" }],
        },
      },
      { type: "tool_call_update", session_id: "s", tool_call: { toolCallId: "t1", status: "completed" } },
      { type: "text_delta", session_id: "s", text: "Hello " },
      { type: "text_delta", session_id: "s", text: "there" },
      { type: "checkpoint", sha: "abcdef1234", summary: "Hello there", files: ["about.md"] },
      { type: "turn_end", session_id: "s", stop_reason: "end_turn" },
    ]);
    await vi.waitFor(() => expect(w.text()).toContain("Hello there"));
    expect(w.text()).toContain("Worked through 2 steps");
    expect(w.text()).toContain("Saved 1 file");
    expect(w.text()).not.toContain("Agent is working");
  });

  it("uploads a picked image and sends its saved path with the message", async () => {
    const w = await mountPanel();
    const input = w.find<HTMLInputElement>("[data-testid='chat-attach-input']");
    const file = new File([new Uint8Array([0x89])], "cat.png", { type: "image/png" });
    Object.defineProperty(input.element, "files", { value: [file], configurable: true });
    await input.trigger("change");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-attachment']").text()).toContain("cat.png"));
    expect(calls.some((c) => c.method === "POST" && c.url === `${ORIGIN}/api/chat/p1/attachments`)).toBe(true);
    await w.find("[data-testid='chat-input']").setValue("use this image");
    await w.find("[data-testid='chat-send']").trigger("click");
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(1));
    expect(textOf(messagePosts()[0]!)).toContain("/state/p1/attachments/abc123.png");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-attachment']").exists()).toBe(false));
  });

  it("groups chat-capable agents and locks the chat's agent after it is created", async () => {
    const w = await mountPanel();
    const picker = agentPicker(w);
    expect(picker.provider()).toBe("cursor");
    await picker.open();
    expect(picker.groups()).toEqual(["cursor", "claude", "codex", "opencode"]);
    await picker.choose("claude:default");
    expect(picker.provider()).toBe("claude");
    await send(w, "hi");
    expect(JSON.parse(String(createPosts()[0]!.init.body)).provider).toBe("claude");
    await picker.open();
    expect(picker.groups()).toEqual(["claude"]);
  });

  it("offers each agent's models after its Default, and sends no model for Default", async () => {
    const w = await mountPanel();
    const picker = agentPicker(w);
    expect(picker.label()).toBe("Cursor");
    await picker.open();
    await vi.waitFor(() =>
      expect(picker.items().filter((v) => v!.startsWith("opencode:"))).toEqual([
        "opencode:default",
        "opencode:opencode-fast",
        "opencode:opencode-smart",
      ]),
    );
    await picker.choose("opencode:default");
    expect(picker.label()).toBe("OpenCode");
    await send(w, "hi");
    const body = JSON.parse(String(createPosts()[0]!.init.body));
    expect(body.provider).toBe("opencode");
    expect(body).not.toHaveProperty("model");
  });

  it("filters models across agents by the search text", async () => {
    const w = await mountPanel();
    const picker = agentPicker(w);
    await picker.open();
    await vi.waitFor(() => expect(picker.items()).toContain("codex:codex-smart"));
    await picker.search("sma");
    expect(picker.items()).toEqual([
      "cursor:cursor-smart",
      "claude:claude-smart",
      "codex:codex-smart",
      "opencode:opencode-smart",
    ]);
    await picker.choose("codex:codex-smart");
    expect(picker.provider()).toBe("codex");
    expect(picker.label()).toBe("Smart");
  });

  it("sends the model picked for a new chat and updates an existing chat's model", async () => {
    const w = await mountPanel();
    const picker = agentPicker(w);
    await picker.open();
    await vi.waitFor(() => expect(picker.items()).toContain("cursor:cursor-smart"));
    await picker.choose("cursor:cursor-smart");
    await send(w, "hi");
    expect(JSON.parse(String(createPosts()[0]!.init.body))).toEqual({ provider: "cursor", model: "cursor-smart" });

    const id = useChatStore().activeChatId("p1")!;
    finish(id, [{ type: "text_delta", session_id: "s", text: "Hello there" }]);
    await vi.waitFor(() => expect(w.text()).toContain("Hello there"));

    await picker.open();
    await picker.choose("cursor:cursor-fast");
    await send(w, "again");
    const model = calls.find((c) => c.method === "POST" && c.url === `${ORIGIN}/api/chat/p1/${id}/model`);
    expect(JSON.parse(String(model!.init.body))).toEqual({ modelId: "cursor-fast" });
    expect(calls.indexOf(model!)).toBeLessThan(calls.indexOf(messagePosts()[1]!));
  });

  it("marks the chat running in the store until chat.turn ends it", async () => {
    const w = await mountPanel();
    const store = useChatStore();
    expect(store.running.p1 ?? null).toBeNull();
    await send(w, "hello");
    const id = store.activeChatId("p1")!;
    expect(store.running.p1).toBe(id);
    finish(id, [{ type: "text_delta", session_id: "s", text: "Done" }]);
    await vi.waitFor(() => expect(store.running.p1).toBeNull());
  });

  it("stops the running turn on the daemon and clears the spinner when it ends", async () => {
    const w = await mountPanel();
    await send(w, "hi");
    const id = useChatStore().activeChatId("p1")!;
    await vi.waitFor(() => expect(w.find("[data-testid='chat-stop']").exists()).toBe(true));
    await w.find("[data-testid='chat-stop']").trigger("click");
    await vi.waitFor(() =>
      expect(calls.some((c) => c.method === "POST" && c.url === `${ORIGIN}/api/chat/p1/${id}/stop`)).toBe(true),
    );
    finish(id, [{ type: "text_delta", session_id: "s", text: "Stopped" }]);
    await vi.waitFor(() => expect(w.find("[data-testid='chat-stop']").exists()).toBe(false));
  });

  it("queues a follow-up on Enter and sends it after the active turn ends", async () => {
    const w = await mountPanel();
    await send(w, "hi");
    const id = useChatStore().activeChatId("p1")!;
    await vi.waitFor(() => expect(w.find("[data-testid='chat-stop']").exists()).toBe(true));
    const input = w.find<HTMLTextAreaElement>("[data-testid='chat-input']");
    await input.setValue("next");
    await input.trigger("keydown", { key: "Enter" });
    await flushPromises();
    expect(messagePosts()).toHaveLength(1);
    expect(input.element.value).toBe("");
    expect(w.get("[data-testid='chat-queued']").text()).toContain("next");
    finish(id, [{ type: "turn_end", session_id: "s", stop_reason: "end_turn" }]);
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(2));
    expect(textOf(messagePosts()[1]!)).toBe("next");
    expect(w.find("[data-testid='chat-queued']").exists()).toBe(false);
  });

  it("keeps queued messages in order and lets one be removed before sending", async () => {
    const w = await mountPanel();
    await send(w, "first");
    const id = useChatStore().activeChatId("p1")!;
    for (const text of ["second", "third", "fourth"]) {
      await w.get("[data-testid='chat-input']").setValue(text);
      await w.get("[data-testid='chat-queue']").trigger("click");
    }
    expect(messagePosts()).toHaveLength(1);
    expect(w.findAll("[data-testid='chat-queued-item']").map(item => item.text())).toEqual([
      expect.stringContaining("second"), expect.stringContaining("third"), expect.stringContaining("fourth"),
    ]);
    await w.findAll("[data-testid='chat-queued-remove']")[1]!.trigger("click");
    finish(id, [{ type: "turn_end", session_id: "s", stop_reason: "end_turn" }]);
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(2));
    expect(textOf(messagePosts()[1]!)).toBe("second");
    finish(id, [{ type: "turn_end", session_id: "s", stop_reason: "end_turn" }]);
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(3));
    expect(textOf(messagePosts()[2]!)).toBe("fourth");
  });

  it("sends a selected queued message next when the current turn finishes", async () => {
    const w = await mountPanel();
    await send(w, "first");
    const id = useChatStore().activeChatId("p1")!;
    for (const text of ["second", "third"]) {
      await w.get("[data-testid='chat-input']").setValue(text);
      await w.get("[data-testid='chat-queue']").trigger("click");
    }
    const sendButtons = w.findAll("[data-testid='chat-queued-send']");
    expect(sendButtons[1]!.attributes("aria-label")).toBe("Send queued message: third");
    await sendButtons[1]!.trigger("click");
    expect(messagePosts()).toHaveLength(1);
    finish(id, [{ type: "turn_end", session_id: "s", stop_reason: "end_turn" }]);
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(2));
    expect(textOf(messagePosts()[1]!)).toBe("third");
    finish(id, [{ type: "turn_end", session_id: "s", stop_reason: "end_turn" }]);
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(3));
    expect(textOf(messagePosts()[2]!)).toBe("second");
  });

  it("pauses queued prompts when the running turn is stopped", async () => {
    const w = await mountPanel();
    await send(w, "first");
    const id = useChatStore().activeChatId("p1")!;
    await w.get("[data-testid='chat-input']").setValue("next");
    await w.get("[data-testid='chat-queue']").trigger("click");
    await w.get("[data-testid='chat-stop']").trigger("click");
    finish(id, [{ type: "turn_end", session_id: "s", stop_reason: "cancelled" }]);
    await flushPromises();
    expect(messagePosts()).toHaveLength(1);
    expect(w.get("[data-testid='chat-queued']").text()).toContain("next");
    await w.get("[data-testid='chat-queue-resume']").trigger("click");
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(2));
    expect(textOf(messagePosts()[1]!)).toBe("next");
  });

  it("pauses a prompt added after Stop and preserves the pause across panel remount", async () => {
    const w = await mountPanel();
    await send(w, "first");
    const id = useChatStore().activeChatId("p1")!;
    await w.get("[data-testid='chat-stop']").trigger("click");
    await w.get("[data-testid='chat-input']").setValue("next");
    await w.get("[data-testid='chat-queue']").trigger("click");
    finish(id, [{ type: "turn_end", session_id: "s", stop_reason: "cancelled" }]);
    await flushPromises();
    expect(messagePosts()).toHaveLength(1);
    w.unmount();
    const reopened = await mountPanel();
    await flushPromises();
    expect(messagePosts()).toHaveLength(1);
    expect(reopened.get("[data-testid='chat-queued']").text()).toContain("next");
    await reopened.get("[data-testid='chat-queue-resume']").trigger("click");
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(2));
  });

  it("keeps a failed queued prompt for retry", async () => {
    const w = await mountPanel();
    await send(w, "first");
    const id = useChatStore().activeChatId("p1")!;
    await w.get("[data-testid='chat-input']").setValue("next");
    await w.get("[data-testid='chat-queue']").trigger("click");
    messageFailure = json({ error: { code: "UNKNOWN", message: "temporary failure" } }, 500);
    finish(id, [{ type: "turn_end", session_id: "s", stop_reason: "end_turn" }]);
    await vi.waitFor(() => expect(w.find("[data-testid='chat-error']").exists()).toBe(true));
    expect(messagePosts()).toHaveLength(2);
    expect(w.get("[data-testid='chat-queued']").text()).toContain("next");
    messageFailure = null;
    await w.get("[data-testid='chat-queue-resume']").trigger("click");
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(3));
    expect(textOf(messagePosts()[2]!)).toBe("next");
  });

  it("queues an uploaded attachment with its prompt", async () => {
    const w = await mountPanel();
    await send(w, "first");
    const id = useChatStore().activeChatId("p1")!;
    const input = w.get<HTMLInputElement>("[data-testid='chat-attach-input']");
    Object.defineProperty(input.element, "files", { value: [new File(["image"], "cat.png", { type: "image/png" })], configurable: true });
    await input.trigger("change");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-attachment']").exists()).toBe(true));
    await w.get("[data-testid='chat-input']").setValue("use this");
    await w.get("[data-testid='chat-queue']").trigger("click");
    expect(messagePosts()).toHaveLength(1);
    finish(id, [{ type: "turn_end", session_id: "s", stop_reason: "end_turn" }]);
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(2));
    expect(textOf(messagePosts()[1]!)).toContain("use this\n/state/p1/attachments/abc123.png");
  });

  it("keeps a queue with its chat and sends it after that chat finishes in the background", async () => {
    const w = await mountPanel();
    await send(w, "first");
    const id = useChatStore().activeChatId("p1")!;
    await w.get("[data-testid='chat-input']").setValue("next");
    await w.get("[data-testid='chat-queue']").trigger("click");
    useChatStore().newChat("p1");
    await flushPromises();
    expect(w.find("[data-testid='chat-queued']").exists()).toBe(false);
    finish(id, [{ type: "turn_end", session_id: "s", stop_reason: "end_turn" }]);
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(2));
    expect(textOf(messagePosts()[1]!)).toBe("next");
    useChatStore().setActive("p1", id);
    await flushPromises();
    expect(messagePosts()).toHaveLength(2);
  });

  it("drains the old chat if selection changes while its turn-end reload is pending", async () => {
    const w = await mountPanel();
    await send(w, "first");
    const id = useChatStore().activeChatId("p1")!;
    await w.get("[data-testid='chat-input']").setValue("next");
    await w.get("[data-testid='chat-queue']").trigger("click");
    let release!: () => void;
    historyGate = new Promise<void>(resolve => { release = resolve; });
    finish(id, [{ type: "turn_end", session_id: "s", stop_reason: "end_turn" }]);
    useChatStore().newChat("p1");
    release();
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(2));
    expect(textOf(messagePosts()[1]!)).toBe("next");
  });

  it("runs turns in two chats independently", async () => {
    const w = await mountPanel();
    await send(w, "first chat");
    const first = useChatStore().activeChatId("p1")!;
    useChatStore().newChat("p1");
    await flushPromises();
    await send(w, "second chat");
    const second = useChatStore().activeChatId("p1")!;
    expect(second).not.toBe(first);
    expect(messagePosts().map(textOf)).toEqual(["first chat", "second chat"]);
    expect(useChatStore().runningChatIds("p1")).toEqual([first, second]);

    finish(first, [{ type: "turn_end", session_id: "s", stop_reason: "end_turn" }]);
    await vi.waitFor(() => expect(useChatStore().runningChatIds("p1")).toEqual([second]));
    expect(w.find("[data-testid='chat-stop']").exists()).toBe(true);
  });

  it("allows another chat to send while the first chat's POST is pending", async () => {
    let release!: () => void;
    messageGate = new Promise<void>(resolve => { release = resolve; });
    const w = await mountPanel();
    await w.get("[data-testid='chat-input']").setValue("first chat");
    await w.get("[data-testid='chat-send']").trigger("click");
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(1));
    const first = useChatStore().activeChatId("p1")!;
    useChatStore().newChat("p1");
    await flushPromises();
    await w.get("[data-testid='chat-input']").setValue("second chat");
    await w.get("[data-testid='chat-send']").trigger("click");
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(2));
    expect(textOf(messagePosts()[1]!)).toBe("second chat");
    expect(useChatStore().activeChatId("p1")).not.toBe(first);
    release();
    await flushPromises();
    expect(w.text()).not.toContain("first chat");
  });

  it("finishes creating a draft in its own session after another chat is selected", async () => {
    chats = [{ id: "existing", provider: "claude", model: null, title: "Existing", updatedAt: 1, live: true, running: false }];
    histories.existing = { chunks: [], provider: "claude", model: null, running: false };
    const draft = useChatStore().newChat("p1");
    let release!: () => void;
    createGate = new Promise<void>(resolve => { release = resolve; });
    const w = await mountPanel();
    await w.get("[data-testid='chat-input']").setValue("draft prompt");
    await w.get("[data-testid='chat-send']").trigger("click");
    await vi.waitFor(() => expect(createPosts()).toHaveLength(1));
    useChatStore().setActive("p1", "existing");
    await flushPromises();
    release();
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(1));
    expect(useChatStore().activeChatId("p1")).toBe("existing");
    expect(messagePosts()[0]!.url).toContain("/c1/message");
    expect(w.text()).not.toContain("draft prompt");
    expect(useChatStore().queued.p1?.[draft]).toBeUndefined();
  });

  it("recovers a queued prompt after reconnecting when the turn event was missed", async () => {
    const w = await mountPanel();
    await send(w, "first");
    const id = useChatStore().activeChatId("p1")!;
    await w.get("[data-testid='chat-input']").setValue("next");
    await w.get("[data-testid='chat-queue']").trigger("click");
    histories[id]!.running = false;
    h.daemon.simulateReconnect();
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(2));
    expect(textOf(messagePosts()[1]!)).toBe("next");
  });

  it("is busy until the draft's history check finishes, and while a turn runs", async () => {
    let release!: () => void;
    historyGate = new Promise((r) => (release = r));
    const w = mount(ChatPanel, { props: { projectId: "p1", active: true }, attachTo: document.body });
    const vm = w.vm as unknown as { busy: boolean; send(t: string): Promise<void> };
    expect(vm.busy).toBe(true);
    release();
    await flushPromises();
    expect(vm.busy).toBe(false);
    void vm.send("hi");
    expect(vm.busy).toBe(true);
    await vi.waitFor(() => expect(w.find("[data-testid='chat-stop']").exists()).toBe(true));
    expect(vm.busy).toBe(true);
  });

  it("offers three suggestions on an empty chat that fill the input", async () => {
    const w = await mountPanel();
    const suggestions = w.findAll("[data-testid='chat-suggestion']");
    expect(suggestions.map((s) => s.text())).toEqual([
      "Write a new blog post about our latest update",
      "Make the homepage headline punchier",
      "Fix typos on the About page",
    ]);
    await suggestions[1]!.trigger("click");
    expect(w.find<HTMLTextAreaElement>("[data-testid='chat-input']").element.value).toBe(
      "Make the homepage headline punchier",
    );
    expect(createPosts()).toHaveLength(0);
  });

  it("shows a refused turn's message in an alert", async () => {
    messageFailure = json({ error: { code: "CONFLICT", message: "A chat turn is already running" } }, 409);
    const w = await mountPanel();
    await send(w, "hi");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-error']").exists()).toBe(true));
    expect(w.find("[data-testid='chat-error']").text()).toBe("A chat turn is already running");
    expect(useChatStore().running.p1 ?? null).toBeNull();
    messageFailure = null;
    await w.get("[data-testid='chat-input']").setValue("try again");
    await w.get("[data-testid='chat-send']").trigger("click");
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(2));
    expect(textOf(messagePosts()[1]!)).toBe("try again");
  });

  it("starts an empty chat with a new active id", async () => {
    histories["c-old"] = {
      chunks: [{ type: "user_text", session_id: "s", text: "Earlier question" }],
      provider: "claude",
      model: null,
      running: false,
    };
    chats = [
      { id: "c-old", provider: "claude", model: null, title: "Old chat", updatedAt: 1, live: false, running: false },
    ];
    useChatStore().setActive("p1", "c-old");
    const w = await mountPanel();
    expect(w.text()).toContain("Earlier question");
    await w.find("[data-testid='chat-new']").trigger("click");
    await flushPromises();
    const id = useChatStore().activeChatId("p1");
    expect(id).not.toBe("c-old");
    expect(id).toBeTruthy();
    expect(w.text()).not.toContain("Earlier question");
    expect(w.findAll("[data-testid='chat-suggestion']")).toHaveLength(3);
  });

  it("loads a chat picked from the history", async () => {
    histories["c-old"] = {
      chunks: [
        { type: "user_text", session_id: "s", text: "Earlier question" },
        { type: "text_delta", session_id: "s", text: "Earlier answer" },
      ],
      provider: "claude",
      model: null,
      running: false,
    };
    chats = [
      { id: "c-old", provider: "claude", model: null, title: "Old chat", updatedAt: 1, live: false, running: false },
    ];
    const w = await mountPanel();
    await w.find("[data-testid='chat-history-toggle']").trigger("click");
    await flushPromises();
    const list = document.body.querySelector("[data-testid='chat-history']")!;
    const item = [...list.querySelectorAll<HTMLButtonElement>("[data-testid='chat-history-item']")].find((b) =>
      b.textContent?.includes("Old chat"),
    )!;
    item.click();
    await flushPromises();
    expect(calls.some((c) => c.method === "GET" && c.url === `${ORIGIN}/api/chat/p1/c-old`)).toBe(true);
    expect(useChatStore().activeChatId("p1")).toBe("c-old");
    await vi.waitFor(() => expect(w.text()).toContain("Earlier answer"));
    expect(agentPicker(w).provider()).toBe("claude");
  });

  it("deletes a chat from the history after confirming", async () => {
    chats = [
      { id: "c-old", provider: "claude", model: null, title: "Old chat", updatedAt: 1, live: false, running: false },
    ];
    histories["c-old"] = { chunks: [], provider: "claude", model: null, running: false };
    const w = await mountPanel();
    await w.find("[data-testid='chat-history-toggle']").trigger("click");
    await flushPromises();
    const list = document.body.querySelector("[data-testid='chat-history']")!;
    const del = list.querySelector<HTMLButtonElement>("[data-testid='chat-history-delete']")!;
    expect(del.getAttribute("aria-label")).toBe("Delete Old chat");
    del.click();
    await flushPromises();
    const deletes = () => calls.filter((c) => c.method === "DELETE" && c.url === `${ORIGIN}/api/chat/p1/c-old`);
    expect(deletes()).toHaveLength(0);
    document.body.querySelector<HTMLButtonElement>("[data-testid='chat-delete-confirm']")!.click();
    await vi.waitFor(() => expect(deletes()).toHaveLength(1));
    await vi.waitFor(() => expect(useChatStore().chats.p1).toEqual([]));
  });

  it("resumes a turn still running when the chat loads", async () => {
    histories["c-live"] = {
      chunks: [{ type: "user_text", session_id: "s", text: "Go" }],
      provider: "cursor",
      model: null,
      running: true,
    };
    chats = [
      { id: "c-live", provider: "cursor", model: null, title: null, updatedAt: 1, live: true, running: true },
    ];
    useChatStore().setActive("p1", "c-live");
    const w = await mountPanel();
    await vi.waitFor(() => expect(w.find("[data-testid='chat-stop']").exists()).toBe(true));
    expect(useChatStore().running.p1).toBe("c-live");
    finish("c-live", [{ type: "text_delta", session_id: "s", text: "Hello there" }]);
    await vi.waitFor(() => expect(w.text()).toContain("Hello there"));
    expect(w.find("[data-testid='chat-stop']").exists()).toBe(false);
  });

  it("shows a loading indicator while the conversation loads", async () => {
    let release!: () => void;
    historyGate = new Promise((r) => (release = r));
    const w = await mountPanel();
    expect(w.find("[data-testid='chat-loading']").exists()).toBe(true);
    expect(w.find("[data-testid='chat-suggestion']").exists()).toBe(false);
    release();
    await flushPromises();
    expect(w.find("[data-testid='chat-loading']").exists()).toBe(false);
    expect(w.findAll("[data-testid='chat-suggestion']")).toHaveLength(3);
  });

  it("keeps the typed text and disables Send while the history loads", async () => {
    let release!: () => void;
    historyGate = new Promise((r) => (release = r));
    const w = await mountPanel();
    await w.find("[data-testid='chat-input']").setValue("hi");
    const sendButton = w.find<HTMLButtonElement>("[data-testid='chat-send']");
    expect(sendButton.element.disabled).toBe(true);
    await sendButton.trigger("click");
    await flushPromises();
    expect(w.find<HTMLTextAreaElement>("[data-testid='chat-input']").element.value).toBe("hi");
    expect(createPosts()).toHaveLength(0);
    release();
    await flushPromises();
    expect(w.find<HTMLButtonElement>("[data-testid='chat-send']").element.disabled).toBe(false);
  });

  it("queues an exposed send until the history has loaded", async () => {
    let release!: () => void;
    historyGate = new Promise((r) => (release = r));
    const w = mount(ChatPanel, { props: { projectId: "p1", active: true }, attachTo: document.body });
    const sent = (w.vm as unknown as { send(t: string): Promise<void> }).send("hi");
    await flushPromises();
    expect(messagePosts()).toHaveLength(0);
    release();
    await sent;
    await flushPromises();
    expect(messagePosts()).toHaveLength(1);
    expect(textOf(messagePosts()[0]!)).toBe("hi");
  });

  it("exposes insert and send for the outbox", async () => {
    const w = await mountPanel();
    const vm = w.vm as unknown as { insert(t: string): void; send(t: string): Promise<void> };
    vm.insert("Fix the footer");
    await flushPromises();
    expect(w.find<HTMLTextAreaElement>("[data-testid='chat-input']").element.value).toBe("Fix the footer");
    await vm.send("Ship it");
    await vi.waitFor(() => expect(messagePosts()).toHaveLength(1));
    expect(textOf(messagePosts()[0]!)).toBe("Ship it");
  });

  it("shows a working indicator and leaves room under the last message while a turn runs", async () => {
    const w = await mountPanel();
    expect(w.find("[data-testid='chat-working']").exists()).toBe(false);
    await send(w, "hi");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-working']").exists()).toBe(true));
    expect(w.find("[data-testid='chat-working']").text()).toBe("Agent is working…");
    expect(w.find("[data-testid='chat-messages']").classes()).toContain("pb-10");
    const id = useChatStore().activeChatId("p1")!;
    finish(id, [{ type: "text_delta", session_id: "s", text: "Done" }]);
    await vi.waitFor(() => expect(w.find("[data-testid='chat-working']").exists()).toBe(false));
    expect(w.find("[data-testid='chat-messages']").classes()).not.toContain("pb-10");
  });

  it("reloads only the active chat when chat.turn fires", async () => {
    histories["c-old"] = {
      chunks: [{ type: "user_text", session_id: "s", text: "Earlier question" }],
      provider: "claude",
      model: null,
      running: false,
    };
    histories["c-other"] = {
      chunks: [{ type: "user_text", session_id: "s", text: "Other" }],
      provider: "claude",
      model: null,
      running: false,
    };
    chats = [
      { id: "c-old", provider: "claude", model: null, title: "Old chat", updatedAt: 1, live: false, running: false },
    ];
    useChatStore().setActive("p1", "c-old");
    const w = await mountPanel();
    expect(w.text()).toContain("Earlier question");
    const before = calls.length;
    h.daemon.emit({ type: "chat.turn", projectId: "p1", chatId: "c-other" });
    await flushPromises();
    expect(calls.slice(before).some((c) => c.url === `${ORIGIN}/api/chat/p1/c-other`)).toBe(false);
    histories["c-old"]!.chunks.push({ type: "text_delta", session_id: "s", text: "Earlier answer" });
    h.daemon.emit({ type: "chat.turn", projectId: "p1", chatId: "c-old" });
    await vi.waitFor(() => expect(w.text()).toContain("Earlier answer"));
  });

  it("fades the message list at its edges under a borderless header", async () => {
    const w = await mountPanel();
    await send(w, "hi");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-viewport']").exists()).toBe(true));
    expect(w.find("[data-testid='chat-viewport']").classes()).toContain("scroll-fade-y");
    expect(w.find("[data-testid='chat-header']").classes()).not.toContain("border-b");
  });

  describe("element references", () => {
    type Vm = { insertReference(t: string): void };
    const ref1 = '[element <section id="hero"> "Stop building" in src/Hero.astro:5:69] /p/attachments/a.png ';
    const ref2 = "[element <h1> at `h1`]";
    const input = (w: Awaited<ReturnType<typeof mountPanel>>) =>
      w.find<HTMLTextAreaElement>("[data-testid='chat-input']");

    it("shows a short numbered tag in the input instead of the full reference", async () => {
      const w = await mountPanel();
      const vm = w.vm as unknown as Vm;
      vm.insertReference(ref1);
      vm.insertReference(ref2);
      await flushPromises();
      expect(input(w).element.value).toBe("[Element #1] [Element #2]");
    });

    it("sends the full reference text in place of each tag", async () => {
      const w = await mountPanel();
      (w.vm as unknown as Vm).insertReference(ref1);
      await flushPromises();
      await send(w, `${input(w).element.value} make it shorter`);
      expect(textOf(messagePosts()[0]!)).toBe(`${ref1.trim()} make it shorter`);
    });

    it("drops a reference whose tag was deleted, and numbers afresh after sending", async () => {
      const w = await mountPanel();
      const vm = w.vm as unknown as Vm;
      vm.insertReference(ref1);
      vm.insertReference(ref2);
      await flushPromises();
      await send(w, "[Element #2] only");
      expect(textOf(messagePosts()[0]!)).toBe(`${ref2} only`);
      vm.insertReference(ref2);
      await flushPromises();
      expect(input(w).element.value).toBe("[Element #1]");
    });
  });
});
