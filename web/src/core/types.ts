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
  | "BADREQ"
  | "GITHUB_ERR"
  | "CHANGE_REQUIRED";

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
  /** `owner/name` when opened from GitHub; null for template projects. */
  repo: string | null;
}

export interface Checkpoint {
  sha: string;
  summary: string;
  /** Commit author name. */
  author: string;
  /** Unix seconds. */
  ts: number;
}

/** Active content-change branch. */
export interface Change {
  branch: string;
  title: string;
  /** Unix seconds of the branch tip commit (list entries). */
  lastEdited?: number;
  /** Set after a successful Share preview. */
  prUrl?: string;
  /** PR state of the change's branch (list entries; only when shared and signed in). */
  prState?: PrState;
}

export type PrState = "draft" | "open" | "approved" | "merged" | "closed";
/** Check runs on the PR head: any failing → failing, else any pending → pending. */
export type ChecksState = "none" | "pending" | "passing" | "failing";

/** `pr` of `GET /api/changes/{branch}/status`. */
export interface PrStatus {
  number: number;
  url: string;
  state: PrState;
  checks: ChecksState;
  /** Deployed preview of the PR head, once the template's workflow published it. */
  previewUrl: string | null;
}

/** Shown after sharing when the repo's `main` has no branch protection. */
export interface ProtectionHint {
  /** The repo's branch protection settings page on GitHub. */
  settingsUrl: string;
  /** The branch protection guide, served by the daemon. */
  docsUrl: string;
}

/** Result of `POST /api/changes/update` (Get latest). */
export interface ChangesUpdate {
  /** Local `main` moved forward from GitHub. */
  mainUpdated: boolean;
  /** Local `main` has commits not on GitHub; it was left as is. */
  mainDiverged: boolean;
  /** Changes that now include latest `main`. */
  updated: Change[];
  /** Changes whose merge conflicts; left untouched. */
  conflicts: Change[];
}

/** Result of `POST /api/changes/{branch}/share`. */
export interface SharedChange {
  branch: string;
  title: string;
  prUrl: string;
  /** Null when `main` is protected, unknown (no admin rights) or the hint was dismissed. */
  protectionHint?: ProtectionHint | null;
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

// ---------------------------------------------------------------------------
// WebSocket events (`/ws/events`) — flat objects tagged by `type`.
// ---------------------------------------------------------------------------

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
  author: string;
  /** Unix seconds. */
  ts: number;
}

export interface FsChangeEvent {
  type: "fs.change";
  projectId: string;
  path: string;
  kind: string;
}

export interface TerminalExitedEvent {
  type: "terminal.exited";
  projectId: string;
  code: number | null;
}

export interface CheckpointBlockedEvent {
  type: "checkpoint.blocked";
  projectId: string;
  reason: string;
}

/** Emitted when this client's subscription fell behind and events were dropped. */
export interface EventsLaggedEvent {
  type: "events.lagged";
  missed: number;
}

export type DaemonEvent =
  | DevserverStatusEvent
  | CheckpointCreatedEvent
  | FsChangeEvent
  | TerminalExitedEvent
  | CheckpointBlockedEvent
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

export interface GithubUser {
  login: string;
  id: number;
  name: string | null;
  avatarUrl: string;
}

export interface GithubStatus {
  /** Some way to sign in or be signed in exists: a client id, or a logged-in GitHub CLI. */
  configured: boolean;
  /** The daemon was started with a GitHub OAuth client id (in-app sign-in is offered). */
  deviceFlow: boolean;
  /** Where the account comes from: an in-app session or the machine's `gh` login. */
  source: "oauth" | "cli" | null;
  user: GithubUser | null;
}

export interface DeviceStart {
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  /** Seconds between polls. */
  interval: number;
}

export type DevicePoll =
  | { status: "pending" }
  | { status: "slowDown"; interval: number }
  | { status: "signedIn"; user: GithubUser }
  | { status: "expired" }
  | { status: "denied" };

export interface GithubRepo {
  fullName: string;
  private: boolean;
  defaultBranch: string;
  canPush: boolean;
}

export interface ContentCollection {
  name: string;
  loader: { base?: string; pattern?: string };
  kind: "json" | "markdown" | "mdx";
  singleton?: string | null;
  editable: boolean;
  reason: string | null;
  /** Field descriptors projected from the template schema. */
  fields?: FieldDescriptor;
}
/** Limited projection of a template schema node; `type` may also be an editor type such as image. */
export interface FieldDescriptor {
  type: "string" | "boolean" | "number" | "date" | "literal" | "enum" | "array" | "object" | "union" | "reference" | "image" | "unsupported";
  required: boolean;
  default?: unknown;
  values?: unknown[];
  items?: FieldDescriptor;
  properties?: Record<string, FieldDescriptor>;
  discriminator?: string;
  variants?: FieldDescriptor[];
  collection?: string;
  reason?: string;
}
export interface ContentEntryUpdate {
  project: string; collection: string; id: string; branch: string; hash: string;
  fields: Record<string, unknown>; body?: string; unset?: string[];
}
export interface ContentEntrySummary { collection: string; id: string; path: string; kind: "json" | "markdown" | "mdx" }
export interface ContentEntry extends ContentEntrySummary {
  source: string; fields: Record<string, unknown> | null; body: string; branch: string; hash: string;
}
