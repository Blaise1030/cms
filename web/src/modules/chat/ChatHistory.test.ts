// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import type { ChatSummary } from "@/core/types";
import ChatHistory from "./ChatHistory.vue";

enableAutoUnmount(afterEach);

function chat(over: Partial<ChatSummary> & Pick<ChatSummary, "id" | "title">): ChatSummary {
  return {
    provider: "cursor",
    kind: "chat",
    model: null,
    updatedAt: Date.parse("2026-11-06T11:26:00Z"),
    live: true,
    running: false,
    ...over,
  };
}

function mountHistory(chats: ChatSummary[], runningIds: string[] = []) {
  return mount(ChatHistory, {
    props: {
      chats,
      activeId: chats[0]?.id ?? null,
      runningIds,
      open: true,
    },
    attachTo: document.body,
  });
}

describe("ChatHistory", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("filters chats by title through the search field", async () => {
    const w = mountHistory([
      chat({ id: "a", title: "Help docs search" }),
      chat({ id: "b", title: "Sync with master" }),
    ]);
    await flushPromises();
    const input = document.body.querySelector<HTMLInputElement>("[data-testid='chat-history-search']")!;
    expect(input).toBeTruthy();
    input.value = "sync";
    input.dispatchEvent(new Event("input"));
    await flushPromises();
    const items = [...document.body.querySelectorAll("[data-testid='chat-history-item']")];
    expect(items).toHaveLength(1);
    expect(items[0]!.textContent).toContain("Sync with master");
  });

  it("shows idle, running, and combining status icons", async () => {
    mountHistory(
      [
        chat({ id: "idle", title: "Idle chat" }),
        chat({ id: "run", title: "Running chat" }),
        chat({
          id: "sync",
          title: "Combining",
          kind: "sync",
          sync: {
            conflictSessionId: "cs1",
            branch: "content/x",
            tip: "a",
            incoming: "b",
            files: [],
            phase: "working",
            startedAt: 1,
            repairAttempted: false,
          },
        }),
      ],
      ["run"],
    );
    await flushPromises();

    expect(
      document.body.querySelector('[data-testid="chat-history-item"][data-status="idle"]'),
    ).toBeTruthy();
    expect(
      document.body.querySelector('[data-testid="chat-history-item"][data-status="running"]'),
    ).toBeTruthy();
    expect(
      document.body.querySelector('[data-testid="chat-history-item"][data-status="combining"]'),
    ).toBeTruthy();
  });

  it("shows no matching chats when the query misses", async () => {
    mountHistory([chat({ id: "a", title: "Help docs" })]);
    await flushPromises();
    const input = document.body.querySelector<HTMLInputElement>("[data-testid='chat-history-search']")!;
    input.value = "zzzz";
    input.dispatchEvent(new Event("input"));
    await flushPromises();
    expect(document.body.textContent).toContain("No matching chats");
    expect(document.body.querySelectorAll("[data-testid='chat-history-item']")).toHaveLength(0);
  });
});
