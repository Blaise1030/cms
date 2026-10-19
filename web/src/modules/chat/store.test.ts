// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useChatStore } from "./store";

function setup(api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return useChatStore();
}

beforeEach(() => {
  vi.restoreAllMocks();
  window.localStorage.clear();
});

describe("chat store", () => {
  it("newChat sets and persists the active chat id", () => {
    const store = setup();
    const id = store.newChat("site");
    expect(id).toMatch(/^[A-Za-z0-9_-]{1,64}$/);
    expect(store.activeChatId("site")).toBe(id);
    expect(window.localStorage.getItem("orso.chat.active.site")).toBe(id);

    setActivePinia(createPinia());
    expect(useChatStore().activeChatId("site")).toBe(id);
    expect(useChatStore().activeChatId("other")).toBeNull();
  });

  it("load fills chats and running for the project", async () => {
    const listChats = vi.fn(async () => ({
      chats: [{ id: "c1", provider: "claude", title: "Hi", updatedAt: 1 }],
      running: "c1",
    }));
    const store = setup({ listChats });
    await store.load("site");
    expect(listChats).toHaveBeenCalledWith("site");
    expect(store.chats.site?.map((c) => c.id)).toEqual(["c1"]);
    expect(store.running.site).toBe("c1");
  });

  it("reloads the list on chat.turn for the project", async () => {
    const listChats = vi.fn(async () => ({ chats: [], running: null }));
    const store = setup({ listChats });
    await store.load("site");
    expect(listChats).toHaveBeenCalledTimes(1);

    h.daemon.emit({ type: "chat.turn", projectId: "site", chatId: "c1" });
    expect(listChats).toHaveBeenCalledTimes(2);
    expect(listChats).toHaveBeenLastCalledWith("site");
  });

  it("remove deletes the chat and starts a new one if it was active", async () => {
    const listChats = vi.fn(async () => ({
      chats: [
        { id: "c1", provider: "claude", title: "One", updatedAt: 2 },
        { id: "c2", provider: "claude", title: "Two", updatedAt: 1 },
      ],
      running: null,
    }));
    const deleteChat = vi.fn(async () => ({ deleted: true }));
    const store = setup({ listChats, deleteChat });
    await store.load("site");
    store.setActive("site", "c1");

    await store.remove("site", "c2");
    expect(deleteChat).toHaveBeenCalledWith("site", "c2");
    expect(store.chats.site?.map((c) => c.id)).toEqual(["c1"]);
    expect(store.activeChatId("site")).toBe("c1");

    await store.remove("site", "c1");
    expect(store.chats.site).toEqual([]);
    expect(store.activeChatId("site")).not.toBe("c1");
    expect(store.activeChatId("site")).toBeTruthy();
  });

  it("remove keeps the chat when the daemon refuses", async () => {
    const listChats = vi.fn(async () => ({
      chats: [{ id: "c1", provider: "claude", title: "One", updatedAt: 1 }],
      running: "c1",
    }));
    const deleteChat = vi.fn(async () => {
      throw new Error("The agent is working in this chat");
    });
    const store = setup({ listChats, deleteChat });
    await store.load("site");
    await expect(store.remove("site", "c1")).rejects.toThrow("working in this chat");
    expect(store.chats.site?.map((c) => c.id)).toEqual(["c1"]);
  });

  it("follows the terminal's session as the active chat", () => {
    const store = setup();
    store.newChat("site");
    h.daemon.emit({ type: "terminal.session", projectId: "site", sessionId: "sess-1" });
    expect(store.activeChatId("site")).toBe("sess-1");
    expect(window.localStorage.getItem("orso.chat.active.site")).toBe("sess-1");
  });
});
