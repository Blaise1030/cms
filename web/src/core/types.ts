// Wire types for the workbench daemon REST + WebSocket contract.
// This file (with api.ts / socket.ts) is the only daemon-aware layer.

export type ErrorCode =
  | "UNAUTH"
  | "SANDBOX"
  | "ENOENT"
  | "CONFLICT"
  | "AGENT_ERR"
  | "DEVSERVER_ERR"
  | "GIT_ERR"
  | "IO"
  | "BADREQ";

/** Codes the daemon documents; anything else maps to `UNKNOWN`. */
export const ERROR_CODES: readonly ErrorCode[] = [
  "UNAUTH",
  "SANDBOX",
  "ENOENT",
  "CONFLICT",
  "AGENT_ERR",
  "DEVSERVER_ERR",
  "GIT_ERR",
  "IO",
  "BADREQ",
];

export type DevserverState = "starting" | "ready" | "crashed";

export interface Project {
  id: string;
  name: string;
  port: number;
  devserver: DevserverState;
  autoApprove: boolean;
}

export interface Provider {
  id: string;
  command: string[];
}

export type ToolState = "running" | "done" | "failed";

export interface ToolSummary {
  tool: string;
  summary: string;
  state: string;
}

export interface Turn {
  id: string;
  sessionId: string;
  provider: string;
  /** Unix seconds. */
  ts: number;
  userText: string;
  assistantText: string;
  tools: ToolSummary[];
  stopReason?: string;
  error?: string;
}

export interface Checkpoint {
  sha: string;
  summary: string;
  /** Unix seconds. */
  ts: number;
}

export interface TreeEntry {
  name: string;
  path: string;
  kind: "file" | "dir";
  size: number | null;
}

export interface Tree {
  path: string;
  entries: TreeEntry[];
}

export interface FileContent {
  path: string;
  size: number;
  content: string | null;
  binary: boolean;
}

export interface Health {
  status: string;
  version: string;
}

export type PermissionDecision = "allow" | "deny";

// ---------------------------------------------------------------------------
// WebSocket events (`/ws/events`) — flat objects tagged by `type`.
// ---------------------------------------------------------------------------

interface SessionScoped {
  projectId?: string;
  sessionId: string;
}

export interface ChatChunkEvent extends SessionScoped {
  type: "chat.chunk";
  turnId: string;
  delta: string;
}

export interface ChatToolEvent extends SessionScoped {
  type: "chat.tool";
  turnId: string;
  toolCallId?: string;
  tool: string;
  state: ToolState;
  summary: string;
}

export interface ChatPlanEvent extends SessionScoped {
  type: "chat.plan";
  turnId: string;
  entries: unknown;
}

export interface ChatDoneEvent extends SessionScoped {
  type: "chat.done";
  turnId: string;
  stopReason?: string;
}

export interface ChatErrorEvent extends SessionScoped {
  type: "chat.error";
  turnId: string;
  message: string;
}

export interface PermissionRequestEvent extends SessionScoped {
  type: "permission.request";
  requestId: string;
  turnId?: string | null;
  tool: string;
  summary?: string;
  command?: string;
}

export interface DevserverStatusEvent {
  type: "devserver.status";
  projectId: string;
  state: DevserverState;
  port?: number;
  message?: string;
}

export interface CheckpointCreatedEvent {
  type: "checkpoint.created";
  projectId: string;
  sha: string;
  summary: string;
}

export interface FsChangeEvent {
  type: "fs.change";
  projectId: string;
  path: string;
  kind: string;
}

/** Emitted when this client's subscription fell behind and events were dropped. */
export interface EventsLaggedEvent {
  type: "events.lagged";
  missed: number;
}

export type DaemonEvent =
  | ChatChunkEvent
  | ChatToolEvent
  | ChatPlanEvent
  | ChatDoneEvent
  | ChatErrorEvent
  | PermissionRequestEvent
  | DevserverStatusEvent
  | CheckpointCreatedEvent
  | FsChangeEvent
  | EventsLaggedEvent;

export type DaemonEventType = DaemonEvent["type"];

export type ConnectionStatus =
  /** First connect attempt in flight. */
  | "connecting"
  | "open"
  /** Lost a live connection; retrying. */
  | "reconnecting"
  /** Could not connect (never opened, or too many consecutive failures). */
  | "unreachable";
