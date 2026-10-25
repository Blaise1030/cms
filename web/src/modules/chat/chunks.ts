import type { DynamicToolUIPart, UIMessage } from "ai";
import type { UiChunk } from "@/core/types";
import type { CheckpointData, ToolInput } from "./parts";

type Part = UIMessage["parts"][number];
type ToolCall = Record<string, unknown>;

/** ACP tool kinds → the labels ChatMessage knows. */
const TOOL_NAMES: Record<string, string> = {
  read: "read",
  edit: "edit",
  execute: "shell",
  search: "search",
  fetch: "web",
};

function toolName(call: ToolCall): string {
  const kind = typeof call.kind === "string" ? call.kind.toLowerCase() : "";
  if (TOOL_NAMES[kind]) return TOOL_NAMES[kind];
  const title = typeof call.title === "string" ? call.title.toLowerCase() : "";
  if (title.startsWith("read")) return "read";
  if (title.includes("edit") || title.includes("write")) return "edit";
  if (title.includes("run") || title.includes("command") || title.includes("terminal")) return "shell";
  if (title.includes("search")) return "search";
  if (title.includes("fetch") || title.includes("web")) return "web";
  return "tool";
}

function toolPart(call: ToolCall, id: string): DynamicToolUIPart {
  const raw = call.rawInput;
  const input: ToolInput = {};
  const locations = Array.isArray(call.locations) ? (call.locations as { path?: unknown }[]) : [];
  const path = locations[0]?.path ?? (raw as { path?: unknown } | null)?.path;
  if (typeof path === "string") input.path = path;
  if (typeof (raw as { command?: unknown } | null)?.command === "string") {
    input.command = (raw as { command: string }).command;
  }
  if (typeof (raw as { query?: unknown } | null)?.query === "string") {
    input.query = (raw as { query: string }).query;
  }
  if (raw !== undefined) input.raw = raw;

  const status = typeof call.status === "string" ? call.status : "";
  const base = {
    type: "dynamic-tool" as const,
    toolName: toolName(call),
    toolCallId: typeof call.toolCallId === "string" ? call.toolCallId : id,
    input,
  };
  if (status === "completed") {
    const output = call.rawOutput ?? call.content;
    return { ...base, state: "output-available", output } as DynamicToolUIPart;
  }
  if (status === "failed") {
    return {
      ...base,
      state: "output-error",
      errorText: typeof call.content === "string" ? call.content : "The tool failed",
    } as DynamicToolUIPart;
  }
  return { ...base, state: "input-available" } as DynamicToolUIPart;
}

/** Rebuild one transcript's chunks as the assistant/user messages ChatMessage renders. */
export function chunksToMessages(chunks: UiChunk[]): UIMessage[] {
  const messages: UIMessage[] = [];
  /** Raw tool calls by id, so a later update can merge into its part. */
  const calls = new Map<string, ToolCall>();
  let assistant: UIMessage | null = null;
  let toolSeq = 0;

  const assistantMessage = (): UIMessage => {
    if (!assistant) {
      assistant = { id: `a-${messages.length}`, role: "assistant", parts: [] };
      messages.push(assistant);
    }
    return assistant;
  };

  for (const chunk of chunks) {
    switch (chunk.type) {
      case "user_text":
        assistant = null;
        messages.push({
          id: `u-${messages.length}`,
          role: "user",
          parts: [{ type: "text", text: chunk.text }],
        });
        break;
      case "text_delta": {
        const message = assistantMessage();
        const last = message.parts.at(-1);
        if (last?.type === "text") last.text += chunk.text;
        else message.parts.push({ type: "text", text: chunk.text });
        break;
      }
      case "thought_delta": {
        const message = assistantMessage();
        const last = message.parts.at(-1);
        if (last?.type === "reasoning") last.text += chunk.text;
        else message.parts.push({ type: "reasoning", text: chunk.text, state: "done" } as Part);
        break;
      }
      case "tool_call": {
        const call = chunk.tool_call;
        const id =
          typeof call.toolCallId === "string" ? call.toolCallId : `tool-${++toolSeq}`;
        calls.set(id, call);
        assistantMessage().parts.push(toolPart(call, id) as Part);
        break;
      }
      case "tool_call_update": {
        const id = typeof chunk.tool_call.toolCallId === "string" ? chunk.tool_call.toolCallId : "";
        if (!id) break;
        const merged = { ...(calls.get(id) ?? {}), ...chunk.tool_call };
        calls.set(id, merged);
        const message = assistantMessage();
        const index = message.parts.findIndex(
          (part) => part.type === "dynamic-tool" && part.toolCallId === id,
        );
        if (index === -1) message.parts.push(toolPart(merged, id) as Part);
        else message.parts[index] = toolPart(merged, id) as Part;
        break;
      }
      case "checkpoint": {
        const data: CheckpointData = {
          sha: chunk.sha ?? undefined,
          summary: chunk.summary ?? undefined,
          files: chunk.files,
          blocked: chunk.blocked ?? undefined,
        };
        assistantMessage().parts.push({ type: "data-checkpoint", data } as Part);
        break;
      }
      case "error":
        assistantMessage().parts.push({
          type: "data-agent-error",
          data: { code: "failed", message: chunk.message },
        } as Part);
        break;
      default:
        // Plans, permissions, turn ends, and future chunk types have no rendering here.
        break;
    }
  }
  return messages;
}
