/** Spike subset of web/src/core/types.ts — only what Shell + Home need. */

export type ErrorCode =
  | "UNAUTH"
  | "SANDBOX"
  | "ENOENT"
  | "CONFLICT"
  | "AGENT_ERR"
  | "DEVSERVER_ERR"
  | "GIT_ERR"
  | "IO"
  | "BADREQ"
  | "GITHUB_ERR"
  | "CHANGE_REQUIRED";

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
  "GITHUB_ERR",
  "CHANGE_REQUIRED",
];

export type DevserverState = "starting" | "ready" | "crashed";

export interface Project {
  id: string;
  name: string;
  port: number;
  devserver: DevserverState;
  autoApprove: boolean;
  repo: string | null;
}

export interface Health {
  status: string;
  version: string;
}

export interface DevserverStatusEvent {
  type: "devserver.status";
  projectId: string;
  state: DevserverState;
  port?: number;
  message?: string;
}

export interface EventsLaggedEvent {
  type: "events.lagged";
  missed: number;
}

/** Spike only listens for these; decode still accepts the full daemon vocabulary. */
export type DaemonEvent =
  | DevserverStatusEvent
  | EventsLaggedEvent
  | { type: "checkpoint.created"; projectId: string; sha: string; summary: string }
  | { type: "fs.change"; projectId: string; path: string; kind: string }
  | { type: "checkpoint.blocked"; projectId: string; reason: string }
  | { type: "changes.updated"; projectId: string }
  | { type: "sync.changed"; projectId: string; conflictSessionId: string; chatId: string; phase: string }
  | { type: "preview.reload"; projectId: string }
  | { type: "preview.deps_installing"; projectId: string }
  | { type: "content.refresh"; projectId: string }
  | { type: "chat.turn"; projectId: string; chatId: string }
  | { type: "chat.permission"; projectId: string; chatId: string }
  | { type: "setup.runtime"; runtime: string; phase: string };

export type DaemonEventType = DaemonEvent["type"];

export type ConnectionStatus = "connecting" | "open" | "reconnecting" | "unreachable";
