import type { DynamicToolUIPart } from "ai";

/** The daemon's normalized tool input: `raw` is the agent's own arguments. */
export interface ToolInput {
  path?: string;
  command?: string;
  query?: string;
  raw?: unknown;
}

export interface CheckpointData {
  sha?: string;
  summary?: string;
  files: string[];
  blocked?: string;
}

export interface AgentErrorData {
  code: "login" | "not_installed" | "failed";
  message: string;
}

const basename = (path = "") => path.split("/").pop() || path;

/** A raw name is only known when `raw` is the name itself, or names it (Codex MCP calls). */
function rawName(raw: unknown): string | undefined {
  if (typeof raw === "string") return raw;
  const tool = (raw as { tool?: unknown } | null | undefined)?.tool;
  return typeof tool === "string" ? tool : undefined;
}

export function toolLabel(part: DynamicToolUIPart): string {
  const input = (part.input ?? {}) as ToolInput;
  switch (part.toolName) {
    case "read":
      return `Read ${basename(input.path)}`;
    case "edit":
      return `Edited ${basename(input.path)}`;
    case "shell":
      return `Ran ${(input.command ?? "").split("\n")[0]!.slice(0, 60)}`;
    case "search":
      return `Searched ${input.query ?? ""}`;
    case "web":
      return "Searched the web";
    default:
      return `Used ${rawName(input.raw) ?? "a tool"}`;
  }
}

/** Strings (diffs, truncated JSON text) show as-is; anything else as pretty JSON. */
export const formatValue = (value: unknown): string =>
  typeof value === "string" ? value : JSON.stringify(value, null, 2);
