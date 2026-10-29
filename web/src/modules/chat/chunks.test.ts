import { describe, expect, it } from "vitest";
import type { DynamicToolUIPart } from "ai";
import type { UiChunk } from "@/core/types";
import { chunksToMessages, createTranscript } from "./chunks";

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

  it("keeps a sync stage line as a data part, verbatim", () => {
    const [, assistant] = chunksToMessages([
      { type: "user_text", session_id: s, text: "go" },
      { type: "sync_status", phase: "working", text: "Reading your pages…" },
    ]);
    expect(assistant!.parts).toEqual([
      {
        type: "data-sync-status",
        data: { phase: "working", text: "Reading your pages…" },
      },
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

describe("createTranscript", () => {
  it("builds the same messages as a full pass, one streamed chunk at a time", () => {
    const chunks = [
      { type: "user_text", session_id: s, text: "hi" },
      { type: "thought_delta", session_id: s, text: "let me look" },
      {
        type: "tool_call",
        session_id: s,
        tool_call: { toolCallId: "t1", kind: "read", status: "pending", locations: [{ path: "a.md" }] },
      },
      {
        type: "tool_call_update",
        session_id: s,
        tool_call: { toolCallId: "t1", status: "completed", content: "done" },
      },
      { type: "text_delta", session_id: s, text: "Hel" },
      { type: "text_delta", session_id: s, text: "lo" },
    ] satisfies UiChunk[];
    const transcript = createTranscript();
    for (const chunk of chunks) transcript.push(chunk);
    expect(transcript.messages).toEqual(chunksToMessages(chunks));
  });

  it("hides agent steps but keeps text and sync status when asked", () => {
    const transcript = createTranscript({ hideSteps: true });
    for (const chunk of [
      { type: "user_text", session_id: s, text: "combine these" },
      { type: "thought_delta", session_id: s, text: "Pondering" },
      {
        type: "tool_call",
        session_id: s,
        tool_call: { toolCallId: "t1", kind: "read", status: "pending", locations: [{ path: "a.md" }] },
      },
      {
        type: "tool_call_update",
        session_id: s,
        tool_call: { toolCallId: "t1", status: "completed", content: "done" },
      },
      { type: "text_delta", session_id: s, text: "Done." },
      { type: "sync_status", phase: "combined", text: "Done. Your changes are combined." },
    ] satisfies UiChunk[]) {
      transcript.push(chunk);
    }
    expect(transcript.messages.map((m) => m.parts)).toEqual([
      [{ type: "text", text: "combine these" }],
      [
        { type: "text", text: "Done." },
        {
          type: "data-sync-status",
          data: { phase: "combined", text: "Done. Your changes are combined." },
        },
      ],
    ]);
  });

  it("keeps agent steps by default", () => {
    const transcript = createTranscript();
    transcript.push({ type: "thought_delta", session_id: s, text: "Pondering" });
    expect(transcript.messages[0]!.parts).toEqual([
      { type: "reasoning", text: "Pondering", state: "done" },
    ]);
  });

  it("appends a delta to the open assistant message in place", () => {
    const transcript = createTranscript();
    transcript.push({ type: "user_text", session_id: s, text: "hi" });
    transcript.push({ type: "text_delta", session_id: s, text: "one" });
    const assistant = transcript.messages.at(-1)!;
    transcript.push({ type: "text_delta", session_id: s, text: " two" });
    expect(transcript.messages).toHaveLength(2);
    expect(transcript.messages.at(-1)).toBe(assistant);
    expect(assistant.parts).toEqual([{ type: "text", text: "one two" }]);
  });
});
