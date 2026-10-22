// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import type { UIMessage } from "ai";
import { createApi } from "@/core/api";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

import ChatPanel from "./ChatPanel.vue";
import { useChatStore } from "./store";

enableAutoUnmount(afterEach);

const ORIGIN = "http://127.0.0.1:7800";
const encoder = new TextEncoder();

type Call = { method: string; url: string; init: RequestInit };
let calls: Call[];
let terminalRunning: boolean;
let histories: Record<string, { messages: UIMessage[]; provider: string; running: boolean }>;
let chatResponse: (init: RequestInit) => Response;
/** When set, chat history responses wait for it. */
let historyGate: Promise<void> | null;

function sse(chunks: unknown[], keepOpen = false, signal?: AbortSignal | null) {
  const body = new ReadableStream<Uint8Array>({
    start(c) {
      for (const chunk of chunks) c.enqueue(encoder.encode(`data: ${JSON.stringify(chunk)}\n\n`));
      if (keepOpen) signal?.addEventListener("abort", () => c.error(new DOMException("aborted", "AbortError")));
      else c.close();
    },
  });
  return new Response(body, { headers: { "content-type": "text/event-stream", "x-vercel-ai-ui-message-stream": "v1" } });
}

const reply = [
  { type: "start", messageId: "a1" },
  { type: "text-start", id: "t1" },
  { type: "text-delta", id: "t1", delta: "Hello " },
  { type: "text-delta", id: "t1", delta: "there" },
  { type: "text-end", id: "t1" },
  { type: "finish" },
];

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

async function fetchMock(input: RequestInfo | URL, init: RequestInit = {}): Promise<Response> {
  const url = String(input);
  const method = (init.method ?? "GET").toUpperCase();
  calls.push({ method, url, init });
  const path = url.replace(ORIGIN, "");
  if (path === "/api/terminal/providers")
    return json({ providers: ["cursor", "claude", "codex", "opencode", "shell"].map((id) => ({ id, command: [id] })) });
  const models = path.match(/^\/api\/terminal\/providers\/([^/]+)\/models$/);
  if (models)
    return json({
      models: [
        { id: `${models[1]}-fast`, name: "Fast" },
        { id: `${models[1]}-smart`, name: "Smart" },
      ],
    });
  if (path === "/api/terminal/p1" && method === "DELETE") {
    terminalRunning = false;
    return json({ stopped: true });
  }
  if (path === "/api/terminal/p1") return json({ running: terminalRunning });
  if (path === "/api/terminal/p1/attachments" && method === "POST") return json({ path: "/state/p1/attachments/abc123.png" });
  if (path === "/api/chat/p1" && method === "GET")
    return json({ chats: [{ id: "c-old", provider: "claude", title: "Old chat", updatedAt: 1 }], running: null });
  if (path === "/api/chat/p1" && method === "POST") return chatResponse(init);
  if (path === "/api/chat/p1/turn" && method === "DELETE") return json({ stopped: true });
  if (path === "/api/chat/p1/c-old" && method === "DELETE") return json({ deleted: true });
  const history = path.match(/^\/api\/chat\/p1\/([^/]+)$/);
  if (history && method === "GET") {
    if (historyGate) await historyGate;
    const found = histories[decodeURIComponent(history[1]!)];
    return found ? json(found) : json({ error: { code: "NOT_FOUND", message: "no such chat" } }, 404);
  }
  return json({ error: { code: "UNKNOWN", message: `unrouted ${method} ${path}` } }, 500);
}

const posts = () => calls.filter((c) => c.method === "POST" && c.url === `${ORIGIN}/api/chat/p1`);

beforeEach(() => {
  localStorage.clear();
  calls = [];
  terminalRunning = false;
  histories = {};
  chatResponse = () => sse(reply);
  historyGate = null;
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
  await vi.waitFor(() => expect(posts().length).toBeGreaterThan(0));
  await flushPromises();
}

/** The agent and model picker: a portalled popover with a searchable list grouped by agent. */
function agentPicker(w: Awaited<ReturnType<typeof mountPanel>>) {
  const trigger = () => w.find("[data-testid='chat-agent']");
  const list = () => [...document.body.querySelectorAll<HTMLElement>("[data-testid='chat-model-item']")];
  return {
    label: () => trigger().text(),
    provider: () => trigger().attributes("data-provider"),
    disabled: () => trigger().attributes("disabled") !== undefined,
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
  it("posts the message with the provider and auth, then renders the streamed reply", async () => {
    const w = await mountPanel();
    await send(w, "hi");
    const post = posts()[0]!;
    const body = JSON.parse(String(post.init.body));
    expect(body.provider).toBe("cursor");
    expect(body.messages.at(-1).parts[0].text).toBe("hi");
    expect(new Headers(post.init.headers).get("Authorization")).toBe("Bearer tok");
    await vi.waitFor(() => expect(w.text()).toContain("Hello there"));
    expect(w.text()).toContain("hi");
  });

  it("uploads a picked image and sends its saved path with the message", async () => {
    const w = await mountPanel();
    const input = w.find<HTMLInputElement>("[data-testid='chat-attach-input']");
    expect(input.exists()).toBe(true);
    const file = new File([new Uint8Array([0x89])], "cat.png", { type: "image/png" });
    Object.defineProperty(input.element, "files", { value: [file], configurable: true });
    await input.trigger("change");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-attachment']").text()).toContain("cat.png"));
    expect(calls.some((c) => c.method === "POST" && c.url === `${ORIGIN}/api/terminal/p1/attachments`)).toBe(true);
    await w.find("[data-testid='chat-input']").setValue("use this image");
    await w.find("[data-testid='chat-send']").trigger("click");
    await vi.waitFor(() => expect(posts()).toHaveLength(1));
    const body = JSON.parse(String(posts()[0]!.init.body));
    expect(body.messages.at(-1).parts[0].text).toContain("/state/p1/attachments/abc123.png");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-attachment']").exists()).toBe(false));
  });

  it("posts only the new message, not the whole conversation", async () => {
    const w = await mountPanel();
    const id = useChatStore().activeChatId("p1");
    await send(w, "hi");
    await vi.waitFor(() => expect(w.text()).toContain("Hello there"));
    chatResponse = () => sse([{ type: "start", messageId: "a2" }, ...reply.slice(1)]);
    await w.find("[data-testid='chat-input']").setValue("again");
    await w.find("[data-testid='chat-send']").trigger("click");
    await vi.waitFor(() => expect(posts()).toHaveLength(2));
    const body = JSON.parse(String(posts()[1]!.init.body));
    expect(body.id).toBe(id);
    expect(body.provider).toBe("cursor");
    expect(body.messages).toHaveLength(1);
    expect(body.messages[0].role).toBe("user");
    expect(body.messages[0].parts[0].text).toBe("again");
    expect(new Headers(posts()[1]!.init.headers).get("Authorization")).toBe("Bearer tok");
  });

  it("groups chat-capable agents and keeps only the chat's agent once it has messages", async () => {
    const w = await mountPanel();
    const picker = agentPicker(w);
    expect(picker.provider()).toBe("cursor");
    await picker.open();
    expect(picker.groups()).toEqual(["cursor", "claude", "codex", "opencode"]);
    await picker.choose("claude:default");
    expect(picker.provider()).toBe("claude");
    await send(w, "hi");
    expect(JSON.parse(String(posts()[0]!.init.body)).provider).toBe("claude");
    await picker.open();
    expect(picker.groups()).toEqual(["claude"]);
    expect(picker.disabled()).toBe(false);
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
    const body = JSON.parse(String(posts()[0]!.init.body));
    expect(body.provider).toBe("opencode");
    expect(body).not.toHaveProperty("model");
  });

  it("filters models across agents by the search text", async () => {
    const w = await mountPanel();
    const picker = agentPicker(w);
    await picker.open();
    await vi.waitFor(() => expect(picker.items()).toContain("codex:codex-smart"));
    await picker.search("sma");
    expect(picker.items()).toEqual(["cursor:cursor-smart", "claude:claude-smart", "codex:codex-smart", "opencode:opencode-smart"]);
    await picker.choose("codex:codex-smart");
    expect(picker.provider()).toBe("codex");
    expect(picker.label()).toBe("Smart");
  });

  it("sends the picked model and lets it change between turns", async () => {
    const w = await mountPanel();
    const picker = agentPicker(w);
    await picker.open();
    await vi.waitFor(() => expect(picker.items()).toContain("cursor:cursor-smart"));
    await picker.choose("cursor:cursor-smart");
    await send(w, "hi");
    expect(JSON.parse(String(posts()[0]!.init.body)).model).toBe("cursor-smart");
    await vi.waitFor(() => expect(w.text()).toContain("Hello there"));

    chatResponse = () => sse([{ type: "start", messageId: "a2" }, ...reply.slice(1)]);
    await picker.open();
    await picker.choose("cursor:cursor-fast");
    await w.find("[data-testid='chat-input']").setValue("again");
    await w.find("[data-testid='chat-send']").trigger("click");
    await vi.waitFor(() => expect(posts()).toHaveLength(2));
    expect(JSON.parse(String(posts()[1]!.init.body)).model).toBe("cursor-fast");
  });

  it("marks the chat as running in the store while a sent turn is in flight", async () => {
    chatResponse = (init) => sse(reply, true, init.signal);
    const w = await mountPanel();
    const store = useChatStore();
    expect(store.running.p1 ?? null).toBeNull();
    await send(w, "hello");
    expect(store.running.p1).toBe(store.activeChatId("p1"));
    expect(store.running.p1).toBeTruthy();
  });

  it("stops a running turn on the client and the daemon", async () => {
    chatResponse = (init) => sse(reply.slice(0, 3), true, init.signal);
    const w = await mountPanel();
    await send(w, "hi");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-stop']").exists()).toBe(true));
    await w.find("[data-testid='chat-stop']").trigger("click");
    await flushPromises();
    expect(calls.some((c) => c.method === "DELETE" && c.url === `${ORIGIN}/api/chat/p1/turn`)).toBe(true);
    await vi.waitFor(() => expect(w.find("[data-testid='chat-stop']").exists()).toBe(false));
  });

  it("doesn't send on Enter while a turn streams, and keeps the typed text", async () => {
    chatResponse = (init) => sse(reply.slice(0, 3), true, init.signal);
    const w = await mountPanel();
    await send(w, "hi");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-stop']").exists()).toBe(true));
    const input = w.find<HTMLTextAreaElement>("[data-testid='chat-input']");
    await input.setValue("next");
    await input.trigger("keydown", { key: "Enter" });
    await flushPromises();
    expect(posts()).toHaveLength(1);
    expect(input.element.value).toBe("next");
  });

  it("is busy until the terminal check completes, and while a turn runs", async () => {
    chatResponse = (init) => sse(reply.slice(0, 3), true, init.signal);
    const w = mount(ChatPanel, { props: { projectId: "p1", active: true }, attachTo: document.body });
    const vm = w.vm as unknown as { busy: boolean; send(t: string): Promise<void> };
    expect(vm.busy).toBe(true);
    await flushPromises();
    expect(vm.busy).toBe(false);
    void vm.send("hi");
    expect(vm.busy).toBe(true);
    await vi.waitFor(() => expect(w.find("[data-testid='chat-stop']").exists()).toBe(true));
    expect(vm.busy).toBe(true);
  });

  it("reloads the history when the stopped turn ends, showing its checkpoint", async () => {
    chatResponse = (init) => sse(reply.slice(0, 3), true, init.signal);
    const w = await mountPanel();
    const id = useChatStore().activeChatId("p1")!;
    await send(w, "hi");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-stop']").exists()).toBe(true));
    await w.find("[data-testid='chat-stop']").trigger("click");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-stop']").exists()).toBe(false));
    histories[id] = {
      provider: "cursor",
      running: false,
      messages: [
        { id: "u1", role: "user", parts: [{ type: "text", text: "hi" }] },
        {
          id: "a1",
          role: "assistant",
          parts: [
            { type: "text", text: "Hello " },
            { type: "data-checkpoint", data: { sha: "abcdef1234", files: ["a.md"] } },
          ],
        },
      ],
    };
    h.daemon.emit({ type: "chat.turn", projectId: "p1", chatId: id });
    await vi.waitFor(() => expect(w.find("[data-testid='checkpoint']").exists()).toBe(true));
    expect(w.find("[data-testid='checkpoint']").text()).toContain("Saved 1 file");
    expect(w.findAll("[data-testid='chat-stop']")).toHaveLength(0);
  });

  it("reloads the history when a terminal turn ends in the active chat", async () => {
    histories["c-old"] = {
      provider: "claude",
      running: false,
      messages: [{ id: "u1", role: "user", parts: [{ type: "text", text: "Earlier question" }] }],
    };
    useChatStore().setActive("p1", "c-old");
    const w = await mountPanel();
    expect(w.text()).not.toContain("Continued in Terminal");
    histories["c-old"].messages.push({ id: "m1", role: "assistant", parts: [{ type: "data-terminal", data: {} }] });
    h.daemon.emit({ type: "terminal.session", projectId: "p1", sessionId: "c-old" });
    await vi.waitFor(() => expect(w.text()).toContain("Continued in Terminal"));
  });

  it("doesn't reload the history over a streaming turn", async () => {
    chatResponse = (init) => sse(reply.slice(0, 3), true, init.signal);
    const w = await mountPanel();
    const id = useChatStore().activeChatId("p1")!;
    await send(w, "hi");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-stop']").exists()).toBe(true));
    const before = calls.length;
    h.daemon.emit({ type: "chat.turn", projectId: "p1", chatId: id });
    await flushPromises();
    expect(calls.slice(before).some((c) => c.url === `${ORIGIN}/api/chat/p1/${id}`)).toBe(false);
    expect(w.text()).toContain("Hello");
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
    expect(posts()).toHaveLength(0);
  });

  it("disables Send while the terminal is running", async () => {
    terminalRunning = true;
    const w = await mountPanel();
    await w.find("[data-testid='chat-input']").setValue("hi");
    expect(w.find<HTMLButtonElement>("[data-testid='chat-send']").element.disabled).toBe(true);
    expect(w.find("[data-testid='chat-send']").attributes("aria-label")).toBe("The agent is busy in Terminal");
  });

  it("offers to stop the terminal session that blocks the chat", async () => {
    terminalRunning = true;
    const w = await mountPanel();
    await w.find("[data-testid='chat-input']").setValue("hi");
    const stopTerminal = w.find("[data-testid='chat-stop-terminal']");
    expect(stopTerminal.text()).toBe("Stop terminal session");
    await stopTerminal.trigger("click");
    await flushPromises();
    expect(calls.some((c) => c.method === "DELETE" && c.url === `${ORIGIN}/api/terminal/p1`)).toBe(true);
    expect(w.find("[data-testid='chat-stop-terminal']").exists()).toBe(false);
    expect(w.find<HTMLButtonElement>("[data-testid='chat-send']").element.disabled).toBe(false);
  });

  it("shows a refused turn's message in an alert", async () => {
    chatResponse = () => json({ error: { code: "CONFLICT", message: "A chat turn is already running" } }, 409);
    const w = await mountPanel();
    await send(w, "hi");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-error']").exists()).toBe(true));
    expect(w.find("[data-testid='chat-error']").text()).toBe("A chat turn is already running");
  });

  it("starts an empty chat with a new active id", async () => {
    histories["c-old"] = {
      provider: "claude",
      running: false,
      messages: [{ id: "u1", role: "user", parts: [{ type: "text", text: "Earlier question" }] }],
    };
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
      provider: "claude",
      running: false,
      messages: [
        { id: "u1", role: "user", parts: [{ type: "text", text: "Earlier question" }] },
        { id: "a1", role: "assistant", parts: [{ type: "text", text: "Earlier answer" }] },
      ],
    };
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
      provider: "cursor",
      running: true,
      messages: [{ id: "u1", role: "user", parts: [{ type: "text", text: "Go" }] }],
    };
    useChatStore().setActive("p1", "c-live");
    (fetch as ReturnType<typeof vi.fn>).mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) =>
      String(input) === `${ORIGIN}/api/chat/p1/c-live/stream` ? sse(reply) : fetchMock(input, init),
    );
    const w = await mountPanel();
    await vi.waitFor(() => expect(w.text()).toContain("Hello there"));
  });

  it("reloads a chat started here when it is reopened", async () => {
    histories["c-old"] = {
      provider: "claude",
      running: false,
      messages: [{ id: "u0", role: "user", parts: [{ type: "text", text: "Earlier question" }] }],
    };
    const w = await mountPanel();
    const id = useChatStore().activeChatId("p1")!;
    await send(w, "hi");
    await vi.waitFor(() => expect(w.text()).toContain("Hello there"));
    histories[id] = {
      provider: "cursor",
      running: false,
      messages: [
        { id: "u1", role: "user", parts: [{ type: "text", text: "hi" }] },
        { id: "a1", role: "assistant", parts: [{ type: "text", text: "Saved reply" }] },
      ],
    };
    useChatStore().setActive("p1", "c-old");
    await vi.waitFor(() => expect(w.text()).toContain("Earlier question"));
    const before = calls.length;
    useChatStore().setActive("p1", id);
    await vi.waitFor(() => expect(w.text()).toContain("Saved reply"));
    expect(calls.slice(before).some((c) => c.method === "GET" && c.url === `${ORIGIN}/api/chat/p1/${id}`)).toBe(true);
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
    await w.find("[data-testid='chat-input']").trigger("keydown", { key: "Enter" });
    await flushPromises();
    expect(w.find<HTMLTextAreaElement>("[data-testid='chat-input']").element.value).toBe("hi");
    expect(posts()).toHaveLength(0);
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
    expect(posts()).toHaveLength(0);
    release();
    await sent;
    await flushPromises();
    expect(posts()).toHaveLength(1);
    expect(JSON.parse(String(posts()[0]!.init.body)).messages.at(-1).parts[0].text).toBe("hi");
  });

  it("exposes insert and send for the outbox", async () => {
    const w = await mountPanel();
    const vm = w.vm as unknown as { insert(t: string): void; send(t: string): Promise<void> };
    vm.insert("Fix the footer");
    await flushPromises();
    expect(w.find<HTMLTextAreaElement>("[data-testid='chat-input']").element.value).toBe("Fix the footer");
    await vm.send("Ship it");
    await vi.waitFor(() => expect(posts()).toHaveLength(1));
    expect(JSON.parse(String(posts()[0]!.init.body)).messages.at(-1).parts[0].text).toBe("Ship it");
  });

  it("shows a working indicator above the input while a turn runs", async () => {
    chatResponse = (init) => sse(reply.slice(0, 3), true, init.signal);
    const w = await mountPanel();
    expect(w.find("[data-testid='chat-working']").exists()).toBe(false);
    await send(w, "hi");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-working']").exists()).toBe(true));
    expect(w.find("[data-testid='chat-working']").text()).toBe("Agent is working…");
    await w.find("[data-testid='chat-stop']").trigger("click");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-working']").exists()).toBe(false));
  });

  it("fades the message list at its edges under a borderless header", async () => {
    const w = await mountPanel();
    await send(w, "hi");
    await vi.waitFor(() => expect(w.find("[data-testid='chat-viewport']").exists()).toBe(true));
    expect(w.find("[data-testid='chat-viewport']").classes()).toContain("scroll-fade-y");
    expect(w.find("[data-testid='chat-header']").classes()).not.toContain("border-b");
  });

  it("leaves room under the last message for the working indicator while a turn runs", async () => {
    chatResponse = (init) => sse(reply.slice(0, 3), true, init.signal);
    const w = await mountPanel();
    await send(w, "hi");
    const list = () => w.find("[data-testid='chat-messages']");
    await vi.waitFor(() => expect(list().classes()).toContain("pb-10"));
    await w.find("[data-testid='chat-stop']").trigger("click");
    await vi.waitFor(() => expect(list().classes()).not.toContain("pb-10"));
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
      const sent = JSON.parse(String(posts()[0]!.init.body)).messages.at(-1).parts[0].text;
      expect(sent).toBe(`${ref1.trim()} make it shorter`);
    });

    it("drops a reference whose tag was deleted, and numbers afresh after sending", async () => {
      const w = await mountPanel();
      const vm = w.vm as unknown as Vm;
      vm.insertReference(ref1);
      vm.insertReference(ref2);
      await flushPromises();
      await send(w, "[Element #2] only");
      expect(JSON.parse(String(posts()[0]!.init.body)).messages.at(-1).parts[0].text).toBe(`${ref2} only`);
      await vi.waitFor(() => expect(w.text()).toContain("Hello there"));
      vm.insertReference(ref2);
      await flushPromises();
      expect(input(w).element.value).toBe("[Element #1]");
    });
  });
});
