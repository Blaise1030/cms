// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import type { ChatSummary } from "@/core/types";
import ChatHistory from "./ChatHistory.vue";

enableAutoUnmount(afterEach);

const stubs = {
  Popover: { template: "<div><slot /></div>" },
  PopoverTrigger: { template: "<div><slot /></div>" },
  PopoverContent: { template: "<div><slot /></div>" },
};

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
    global: { stubs },
  });
}

describe("ChatHistory", () => {
  it("filters chats by title through the search field", async () => {
    const w = mountHistory([
      chat({ id: "a", title: "Help docs search" }),
      chat({ id: "b", title: "Sync with master" }),
    ]);
    await w.get("[data-testid='chat-history-search']").setValue("sync");
    await flushPromises();
    const items = w.findAll("[data-testid='chat-history-item']");
    expect(items).toHaveLength(1);
    expect(items[0]!.text()).toContain("Sync with master");
  });

  it("shows idle, running, and combining status icons", async () => {
    const w = mountHistory(
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

    expect(w.find('[data-testid="chat-history-item"][data-status="idle"]').exists()).toBe(true);
    expect(w.find('[data-testid="chat-history-item"][data-status="running"]').exists()).toBe(true);
    expect(w.find('[data-testid="chat-history-item"][data-status="combining"]').exists()).toBe(true);
  });

  it("shows no matching chats when the query misses", async () => {
    const w = mountHistory([chat({ id: "a", title: "Help docs" })]);
    await w.get("[data-testid='chat-history-search']").setValue("zzzz");
    await flushPromises();
    expect(w.text()).toContain("No matching chats");
    expect(w.findAll("[data-testid='chat-history-item']")).toHaveLength(0);
  });
});
