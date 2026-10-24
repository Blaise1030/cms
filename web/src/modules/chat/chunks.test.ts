import { describe, expect, it } from "vitest";
import type { DynamicToolUIPart } from "ai";
import { chunksToMessages } from "./chunks";

const s = "session-1";

describe("chunksToMessages", () => {
  it("turns user echoes and text deltas into user and assistant messages", () => {
    const messages = chunksToMessages([
      { type: "user_text", session_id: s, text: "hi" },
      { type: "text_delta", session_id: s, text: "Hello " },
      { type: "text_delta", session_id: s, text: "there" },
    ]);
    expect(messages).toHaveLength(2);
    expect(messages[0]).toEqual({
      id: "u-0",
      role: "user",
      parts: [{ type: "text", text: "hi" }],
    });
    expect(messages[1]).toMatchObject({ id: "a-1", role: "assistant" });
    expect(messages[1]!.parts).toEqual([{ type: "text", text: "Hello there" }]);
  });

  it("starts a new assistant message after each user turn", () => {
    const messages = chunksToMessages([
      { type: "user_text", session_id: s, text: "one" },
      { type: "text_delta", session_id: s, text: "first" },
      { type: "user_text", session_id: s, text: "two" },
      { type: "text_delta", session_id: s, text: "second" },
    ]);
    expect(messages.map((m) => m.role)).toEqual(["user", "assistant", "user", "assistant"]);
    expect(messages.map((m) => (m.parts[0] as { text: string }).text)).toEqual([
      "one",
      "first",
      "two",
      "second",
    ]);
  });

  it("merges reasoning deltas into one done reasoning part", () => {
    const [, assistant] = chunksToMessages([
      { type: "user_text", session_id: s, text: "go" },
      { type: "thought_delta", session_id: s, text: "Think" },
      { type: "thought_delta", session_id: s, text: "ing" },
    ]);
    expect(assistant!.parts).toEqual([{ type: "reasoning", text: "Thinking", state: "done" }]);
  });

  it("labels an ACP tool by its kind and merges a later update", () => {
    const [, assistant] = chunksToMessages([
      { type: "user_text", session_id: s, text: "edit it" },
      {
        type: "tool_call",
        session_id: s,
        tool_call: {
          toolCallId: "t1",
          kind: "edit",
          title: "Edit about.md",
          status: "pending",
          locations: [{ path: "about.md" }],
        },
      },
      {
        type: "tool_call_update",
        session_id: s,
        tool_call: { toolCallId: "t1", status: "completed", rawOutput: { ok: true } },
      },
    ]);
    expect(assistant!.parts).toHaveLength(1);
    const tool = assistant!.parts[0] as DynamicToolUIPart;
    expect(tool).toMatchObject({
      type: "dynamic-tool",
      toolName: "edit",
      toolCallId: "t1",
      state: "output-available",
      input: { path: "about.md" },
      output: { ok: true },
    });
  });

  it("marks a failed tool as an error and keeps its message", () => {
    const [, assistant] = chunksToMessages([
      { type: "user_text", session_id: s, text: "run it" },
      {
        type: "tool_call",
        session_id: s,
        tool_call: { toolCallId: "t1", kind: "execute", status: "failed", content: "exit 1" },
      },
    ]);
    expect(assistant!.parts[0]).toMatchObject({
      type: "dynamic-tool",
      toolName: "shell",
      state: "output-error",
      errorText: "exit 1",
    });
  });

  it("keeps checkpoints and agent errors as data parts", () => {
    const [, assistant] = chunksToMessages([
      { type: "user_text", session_id: s, text: "go" },
      { type: "checkpoint", sha: "abc", summary: "Done", files: ["a.md"] },
      { type: "error", message: "agent exited" },
    ]);
    expect(assistant!.parts).toEqual([
      {
        type: "data-checkpoint",
        data: { sha: "abc", summary: "Done", files: ["a.md"], blocked: undefined },
      },
      { type: "data-agent-error", data: { code: "failed", message: "agent exited" } },
    ]);
  });

  it("ignores plans, permissions, unknown updates, and turn ends", () => {
    const messages = chunksToMessages([
      { type: "user_text", session_id: s, text: "go" },
      { type: "plan", session_id: s, entries: [{ content: "step" }] },
      { type: "permission", session_id: s, request_id: "p1", tool_call: {}, options: [] },
      { type: "unknown", session_id: s, update: {} },
      { type: "turn_end", session_id: s, stop_reason: "end_turn" },
    ]);
    expect(messages).toHaveLength(1);
    expect(messages[0]!.parts).toEqual([{ type: "text", text: "go" }]);
  });
});
