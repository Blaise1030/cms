import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { agentPrompt, useAgentOutboxStore } from "./outbox";

beforeEach(() => setActivePinia(createPinia()));

describe("agent outbox", () => {
  it("drains queued messages in order and empties the queue", () => {
    const outbox = useAgentOutboxStore();
    outbox.send({ kind: "text", text: "one" });
    outbox.send({ kind: "text", text: "two" });
    expect(outbox.drain()).toEqual([
      { kind: "text", text: "one" },
      { kind: "text", text: "two" },
    ]);
    expect(outbox.messages).toEqual([]);
  });

  it("renders text as-is and elements as their reference", () => {
    expect(agentPrompt({ kind: "text", text: "fix the header" })).toBe("fix the header");
    expect(
      agentPrompt({ kind: "element", element: { tag: "<h1>", selector: "h1", text: "Hi" } as never }),
    ).toBe('[element <h1> "Hi" at `h1`]');
  });
});
