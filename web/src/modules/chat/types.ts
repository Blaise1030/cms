import type { ToolState } from "@/core/types";

export type TurnStatus = "streaming" | "done" | "error";

export interface ToolView {
  /** `toolCallId` when the agent provides one, otherwise a local key. */
  id: string;
  tool: string;
  summary: string;
  state: ToolState;
}

export interface TurnView {
  /** Daemon turn id, or a `local-N` placeholder until the first event / prompt response. */
  id: string;
  /** True while `id` is still a local placeholder. */
  pending: boolean;
  sessionId: string | null;
  /** Unix seconds. */
  ts: number;
  userText: string;
  assistantText: string;
  tools: ToolView[];
  status: TurnStatus;
  stopReason?: string;
  error?: string;
}

export interface PermissionView {
  requestId: string;
  sessionId: string;
  turnId: string | null;
  tool: string;
  summary?: string;
  command?: string;
  resolving: boolean;
}

export interface ProjectChat {
  sessionId: string | null;
  provider: string | null;
  turns: TurnView[];
  permissions: PermissionView[];
  /** Session-level failure (agent died with no active turn). */
  sessionError: string | null;
  /** Last send/permission failure worth showing inline. */
  error: string | null;
  historyLoaded: boolean;
  loadingHistory: boolean;
  /** A prompt request is in flight (session start + POST). */
  sending: boolean;
  /** Prompt text waiting on "Start a change first" (GitHub project on main). */
  changeRequiredPrompt: string | null;
}
