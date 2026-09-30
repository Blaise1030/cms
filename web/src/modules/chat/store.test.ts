// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { ApiError } from "@/core/api";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { Turn } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useChatStore } from "./store";

type Api = Record<string, ReturnType<typeof vi.fn>>;
let api: Api;

function setup(over: Api = {}) {
  api = {
    listAgents: vi.fn(async () => [{ id: "claude", command: ["x"] }]),
    history: vi.fn(async () => []),
    startSession: vi.fn(async () => "s1"),
    prompt: vi.fn(async () => "t1"),
    cancelTurn: vi.fn(async () => ({})),
    resolvePermission: vi.fn(async () => ({})),
    ...over,
  };
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return useChatStore();
}

beforeEach(() => localStorage.clear());

async function started(store = setup()) {
  await store.openProject("p");
  await store.send("p", "make the hero blue");
  return store;
}

describe("chat store", () => {
  it("restores turns from history", async () => {
    const turn: Turn = {
      id: "t0",
      sessionId: "old",
      provider: "claude",
      ts: 10,
      userText: "hi",
      assistantText: "hello",
      tools: [{ tool: "edit", summary: "edit a.mdx +1 -1", state: "completed" }],
    };
    const store = setup({ history: vi.fn(async () => [turn]) });
    await store.openProject("p");
    const pc = store.chat("p");
    expect(api.history).toHaveBeenCalledWith("p");
    expect(pc.turns).toHaveLength(1);
    expect(pc.turns[0]).toMatchObject({ status: "done", assistantText: "hello" });
    expect(pc.turns[0]!.tools[0]!.state).toBe("done");
  });

  it("starts a session lazily on first send and streams the reply", async () => {
    const store = await started();
    expect(api.startSession).toHaveBeenCalledWith("p", "claude");
    expect(api.prompt).toHaveBeenCalledWith("p", "s1", "make the hero blue");
    const turn = store.chat("p").turns[0]!;
    expect(turn).toMatchObject({ id: "t1", pending: false, status: "streaming" });

    h.daemon.emit({ type: "chat.chunk", sessionId: "s1", turnId: "t1", delta: "Sure, " });
    h.daemon.emit({ type: "chat.chunk", sessionId: "s1", turnId: "t1", delta: "done." });
    expect(turn.assistantText).toBe("Sure, done.");
    expect(store.isBusy("p")).toBe(true);

    h.daemon.emit({ type: "chat.done", sessionId: "s1", turnId: "t1", stopReason: "end_turn" });
    expect(turn.status).toBe("done");
    expect(store.isBusy("p")).toBe(false);
  });

  it("does not start a second session for later prompts", async () => {
    const store = await started();
    h.daemon.emit({ type: "chat.done", sessionId: "s1", turnId: "t1" });
    api.prompt!.mockResolvedValueOnce("t2");
    await store.send("p", "again");
    expect(api.startSession).toHaveBeenCalledTimes(1);
    expect(store.chat("p").turns).toHaveLength(2);
  });

  it("adopts events that arrive before the prompt response", async () => {
    let resolvePrompt!: (id: string) => void;
    const store = setup({
      prompt: vi.fn(() => new Promise<string>((r) => (resolvePrompt = r))),
    });
    await store.openProject("p");
    const sending = store.send("p", "go");
    await vi.waitFor(() => expect(api.prompt).toHaveBeenCalled());
    h.daemon.emit({ type: "chat.chunk", sessionId: "s1", turnId: "early", delta: "hi" });
    resolvePrompt("early");
    await sending;
    const turns = store.chat("p").turns;
    expect(turns).toHaveLength(1);
    expect(turns[0]).toMatchObject({ id: "early", pending: false, assistantText: "hi", userText: "go" });
  });

  it("tracks tool calls and updates them by toolCallId", async () => {
    const store = await started();
    h.daemon.emit({
      type: "chat.tool", sessionId: "s1", turnId: "t1", toolCallId: "c1",
      tool: "edit", state: "running", summary: "edit hero.mdx",
    });
    h.daemon.emit({
      type: "chat.tool", sessionId: "s1", turnId: "t1", toolCallId: "c1",
      tool: "edit", state: "done", summary: "edit hero.mdx +12 -3",
    });
    const tools = store.chat("p").turns[0]!.tools;
    expect(tools).toEqual([
      { id: "c1", tool: "edit", summary: "edit hero.mdx +12 -3", state: "done" },
    ]);
  });

  it("queues a permission prompt and resolves it with Allow", async () => {
    const store = await started();
    h.daemon.emit({
      type: "permission.request", sessionId: "s1", requestId: "r1", turnId: "t1",
      tool: "bash", command: "npm run build",
    });
    const pc = store.chat("p");
    expect(pc.permissions).toHaveLength(1);
    expect(pc.permissions[0]).toMatchObject({ requestId: "r1", command: "npm run build" });

    await store.resolvePermission("p", "r1", "allow");
    expect(api.resolvePermission).toHaveBeenCalledWith("r1", "allow");
    expect(pc.permissions).toHaveLength(0);
  });

  it("resolves a permission prompt with Deny", async () => {
    const store = await started();
    h.daemon.emit({ type: "permission.request", sessionId: "s1", requestId: "r2", tool: "bash" });
    await store.resolvePermission("p", "r2", "deny");
    expect(api.resolvePermission).toHaveBeenCalledWith("r2", "deny");
  });

  it("keeps the prompt and reports an error when resolving fails", async () => {
    const store = await started();
    api.resolvePermission!.mockRejectedValueOnce(new Error("gone"));
    h.daemon.emit({ type: "permission.request", sessionId: "s1", requestId: "r3", tool: "bash" });
    await store.resolvePermission("p", "r3", "allow");
    const pc = store.chat("p");
    expect(pc.permissions).toHaveLength(1);
    expect(pc.permissions[0]!.resolving).toBe(false);
    expect(pc.error).toBe("gone");
  });

  it("stop cancels the streaming turn", async () => {
    const store = await started();
    await store.stop("p");
    expect(api.cancelTurn).toHaveBeenCalledWith("t1");
    h.daemon.emit({ type: "chat.done", sessionId: "s1", turnId: "t1", stopReason: "cancelled" });
    expect(store.chat("p").turns[0]!.stopReason).toBe("cancelled");
    expect(store.isBusy("p")).toBe(false);
  });

  it("marks only the failing turn on a turn-scoped chat.error", async () => {
    const store = await started();
    h.daemon.emit({ type: "chat.error", sessionId: "s1", turnId: "t1", message: "rate limited" });
    const pc = store.chat("p");
    expect(pc.turns[0]).toMatchObject({ status: "error", error: "rate limited" });
    expect(pc.sessionError).toBeNull();
    expect(pc.sessionId).toBe("s1");
  });

  it("treats chat.error with null turnId as session death", async () => {
    const store = await started();
    h.daemon.emit({ type: "permission.request", sessionId: "s1", requestId: "r9", tool: "bash" });
    h.daemon.emit({ type: "chat.error", sessionId: "s1", turnId: null, message: "agent exited" });
    const pc = store.chat("p");
    expect(pc.sessionError).toBe("agent exited");
    expect(pc.sessionId).toBeNull();
    expect(pc.permissions).toHaveLength(0);
    expect(pc.turns[0]).toMatchObject({ status: "error" });
    expect(store.isBusy("p")).toBe(false);

    // Restart begins a fresh session.
    api.startSession!.mockResolvedValueOnce("s2");
    await store.restartSession("p");
    expect(pc.sessionId).toBe("s2");
    expect(pc.sessionError).toBeNull();
  });

  it("surfaces a send failure on the turn", async () => {
    const store = setup({ prompt: vi.fn(async () => { throw new Error("nope"); }) });
    await store.openProject("p");
    await store.send("p", "hi");
    expect(store.chat("p").turns[0]).toMatchObject({ status: "error", error: "nope" });
    expect(store.isBusy("p")).toBe(false);
  });

  it("recovers from a stale remembered session by starting a new one", async () => {
    localStorage.setItem("workbench.session.p", "dead");
    const store = setup({
      startSession: vi.fn(async () => "fresh"),
      prompt: vi
        .fn()
        .mockRejectedValueOnce(new ApiError("ENOENT", "no active agent session", 404))
        .mockResolvedValueOnce("t9"),
    });
    await store.openProject("p");
    await store.send("p", "hi");
    expect(api.prompt).toHaveBeenNthCalledWith(1, "p", "dead", "hi");
    expect(api.prompt).toHaveBeenNthCalledWith(2, "p", "fresh", "hi");
    expect(store.chat("p").turns[0]).toMatchObject({ id: "t9", status: "streaming" });
  });

  it("restarts a session that died unnoticed (missed chat.error) on the next prompt", async () => {
    const store = await started();
    h.daemon.emit({ type: "chat.done", sessionId: "s1", turnId: "t1" });
    // The agent died while the socket was down; the daemon no longer knows s1.
    h.daemon.simulateReconnect();
    api.prompt!
      .mockRejectedValueOnce(new ApiError("ENOENT", "no active agent session", 404))
      .mockResolvedValueOnce("t2");
    api.startSession!.mockResolvedValueOnce("s2");
    await store.send("p", "again");
    expect(api.prompt).toHaveBeenNthCalledWith(2, "p", "s1", "again");
    expect(api.prompt).toHaveBeenNthCalledWith(3, "p", "s2", "again");
    const pc = store.chat("p");
    expect(pc.sessionId).toBe("s2");
    expect(pc.turns.find((t) => t.id === "t2")).toMatchObject({ status: "streaming" });
  });

  it("shows the restart banner when the session cannot be recovered", async () => {
    const store = await started();
    h.daemon.emit({ type: "chat.done", sessionId: "s1", turnId: "t1" });
    api.prompt!.mockRejectedValue(new ApiError("AGENT_ERR", "agent session died", 500));
    api.startSession!.mockResolvedValueOnce("s2");
    await store.send("p", "again");
    const pc = store.chat("p");
    expect(pc.turns[1]).toMatchObject({ status: "error" });
    expect(pc.sessionError).toBe("agent session died");
    expect(pc.sessionId).toBeNull();
    // Restart clears the banner and creates a session.
    api.startSession!.mockResolvedValueOnce("s3");
    await store.restartSession("p");
    expect(pc.sessionError).toBeNull();
    expect(pc.sessionId).toBe("s3");
  });

  it("does not retry non-session errors", async () => {
    const store = await started();
    h.daemon.emit({ type: "chat.done", sessionId: "s1", turnId: "t1" });
    api.prompt!.mockRejectedValueOnce(new ApiError("CONFLICT", "turn already running", 409));
    await store.send("p", "again");
    expect(api.startSession).toHaveBeenCalledTimes(1);
    expect(store.chat("p").sessionError).toBeNull();
    expect(store.chat("p").sessionId).toBe("s1");
  });

  it("re-syncs history when the server reports events.lagged", async () => {
    const store = await started();
    api.history!.mockResolvedValueOnce([
      {
        id: "t1", sessionId: "s1", provider: "claude", ts: 1, userText: "make the hero blue",
        assistantText: "missed reply", tools: [],
      },
    ]);
    h.daemon.emit({ type: "events.lagged", missed: 12 });
    await vi.waitFor(() => expect(store.chat("p").turns[0]!.status).toBe("done"));
    expect(store.chat("p").turns[0]!.assistantText).toBe("missed reply");
  });

  it("ignores events for unknown sessions", async () => {
    const store = await started();
    h.daemon.emit({ type: "chat.chunk", sessionId: "other", turnId: "x", delta: "no" });
    expect(store.chat("p").turns).toHaveLength(1);
    expect(store.chat("p").turns[0]!.assistantText).toBe("");
  });

  it("does not send while a turn is streaming or when text is blank", async () => {
    const store = await started();
    await store.send("p", "second");
    await store.send("p", "   ");
    expect(api.prompt).toHaveBeenCalledTimes(1);
  });

  it("re-syncs history after reconnect, keeping unsettled streaming turns", async () => {
    const store = await started();
    api.history!.mockResolvedValueOnce([
      {
        id: "t1", sessionId: "s1", provider: "claude", ts: 1, userText: "make the hero blue",
        assistantText: "all done", tools: [],
      },
    ]);
    h.daemon.simulateReconnect();
    await vi.waitFor(() => expect(store.chat("p").turns[0]!.status).toBe("done"));
    expect(store.chat("p").turns[0]!.assistantText).toBe("all done");
  });

  it("changing provider drops the session so the next prompt starts fresh", async () => {
    const store = setup({ listAgents: vi.fn(async () => [{ id: "claude", command: [] }, { id: "codex", command: [] }]) });
    await store.openProject("p");
    await store.send("p", "one");
    h.daemon.emit({ type: "chat.done", sessionId: "s1", turnId: "t1" });
    store.setProvider("p", "codex");
    api.startSession!.mockResolvedValueOnce("s2");
    api.prompt!.mockResolvedValueOnce("t2");
    await store.send("p", "two");
    expect(api.startSession).toHaveBeenLastCalledWith("p", "codex");
  });
});
