// Wire types for the orso daemon REST + WebSocket contract.
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

/** A commit that touched one content entry's file, newest first. */
export interface FileVersion {
  sha: string;
  summary: string;
  author: string;
  /** Unix seconds. */
  ts: number;
  /** The entry's path at that commit (it may have been renamed since). */
  path: string;
  published: boolean;
}

/** One content entry as it was at a commit. */
export interface ContentVersion {
  sha: string;
  path: string;
  source: string;
  fields: Record<string, unknown>;
  body: string;
  parseable: boolean;
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
  /** GitHub can merge the PR as it stands; null while GitHub is still working it out. */
  mergeable: boolean | null;
  /** GitHub's finer-grained merge state (`clean`, `dirty`, `blocked`, `behind`, …). */
  mergeableState: string | null;
  /** The local branch tip is the PR head: there is nothing new to push. */
  upToDate: boolean;
}

/** Whether the deploy a publish set off has shipped. */
export type DeployState = "unknown" | "building" | "live" | "failed";

/** `state` of `GET /api/projects/{id}/deploy`. */
export interface DeployStatus {
  state: DeployState;
}

/** A merged pull request, with the commit whose deploy ships the change. */
export interface PublishedPr extends PrStatus {
  /** The merge commit on the site's main line; absent when GitHub didn't report one. */
  mergeSha?: string | null;
}

/** Push needed newer commits from this change's remote copy combined in first. */
export interface PendingCombine {
  branch: string;
  sessionId: string;
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
  /** The current change, left as is because an agent is running. */
  skipped?: Change | null;
  /** Branch → resolution session. Older daemons may omit this field. */
  conflictSessions?: Record<string, string>;
  /** Conflicts the visual resolver cannot safely open. */
  conflictErrors?: Record<string, string>;
}

export type ConflictChoice = "yours" | "latest" | "both";

export interface ConflictFile {
  path: string;
  binary: boolean;
  oursDeleted?: boolean;
  theirsDeleted?: boolean;
  oursOid: string;
  theirsOid: string;
  baseOid: string | null;
  patch?: string;
}

export interface ConflictSession {
  id: string;
  projectId: string;
  branch: string;
  tip: string;
  incoming: string;
  base: string;
  expiresAt: number;
  choices: Record<string, ConflictChoice>;
  files: ConflictFile[];
  candidate?: ConflictCandidate;
}

export interface ConflictCandidate {
  tree: string;
  complete: boolean;
}

export type ConflictPreviewState =
  | { status: "ready"; url: string }
  | { status: "unavailable"; reason: string }
  | { status: "invalid"; errors: string };

export interface ConflictPreviews {
  yours: ConflictPreviewState;
  latest: ConflictPreviewState;
  result: ConflictPreviewState;
}

/** Result of `POST /api/changes/{branch}/share`. */
export interface SharedChange {
  branch: string;
  title: string;
  /** Null when the push needs combining with newer commits from the remote copy. */
  prUrl: string | null;
  /** Set instead of `prUrl` when someone else moved this change first. */
  conflict?: PendingCombine;
  /** Null when `main` is protected, unknown (no admin rights) or the hint was dismissed. */
  protectionHint?: ProtectionHint | null;
}

/** Result of `POST /api/changes/{branch}/publish`: the merged PR, or a combine to finish. */
export type PublishResult = PublishedPr | null | { conflict: PendingCombine };

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

/** A chat conversation, from `GET /api/chat/{project}`. */
export interface ChatSummary {
  id: string;
  provider: string;
  /** Whether this chat is a conversation or a combine of a draft with the live site. */
  kind: "chat" | "sync";
  model?: string | null;
  title?: string | null;
  /** Unix milliseconds. */
  updatedAt: number;
  /** The ACP agent process is up (a turn may or may not be running). */
  live: boolean;
  /** A prompt is in flight. */
  running: boolean;
  /** The combine this sync chat is doing; absent or null for ordinary chats. */
  sync?: SyncState | null;
}

/** Where a sync chat is; mirrors the backend `SyncPhase`. */
export type SyncPhase = "working" | "checking" | "combined" | "ended" | "expired";

/** The combine a sync chat is doing: its conflict session, the two sides, and its phase. */
export interface SyncState {
  conflictSessionId: string;
  branch: string;
  tip: string;
  incoming: string;
  files: string[];
  phase: SyncPhase;
  /** Unix milliseconds. */
  startedAt: number;
  /** Unix milliseconds; present once the chat reached a terminal phase. */
  endedAt?: number;
  /** The one automatic repair after a failed build has been attempted. */
  repairAttempted: boolean;
}

/** One line of a chat transcript, as `GET /api/chat/{project}/{chat}` returns it. */
export type UiChunk =
  | { type: "text_delta"; session_id: string; text: string }
  | { type: "thought_delta"; session_id: string; text: string }
  | { type: "user_text"; session_id: string; text: string }
  | { type: "tool_call"; session_id: string; tool_call: Record<string, unknown> }
  | { type: "tool_call_update"; session_id: string; tool_call: Record<string, unknown> }
  | { type: "plan"; session_id: string; entries: unknown }
  | {
      type: "permission";
      session_id: string;
      request_id: string;
      tool_call: Record<string, unknown>;
      options: unknown;
    }
  | { type: "turn_end"; session_id: string; stop_reason: string }
  | { type: "unknown"; session_id: string; update: unknown }
  | { type: "error"; message: string }
  | {
      type: "checkpoint";
      sha?: string | null;
      summary?: string | null;
      files: string[];
      blocked?: string | null;
    }
  | { type: "sync_status"; phase: SyncPhase; text: string };

/** `GET /api/chat/{project}/{chat}`. */
export interface ChatHistory {
  chunks: UiChunk[];
  provider: string;
  model: string | null;
  running: boolean;
  /** Request ids of permission prompts the agent is still blocked on. */
  pendingPermissions?: string[];
}

/** The agent asked permission mid-turn; refetch the chat to show the card. */
export interface ChatPermissionEvent {
  type: "chat.permission";
  projectId: string;
  chatId: string;
}

/** A chat turn ended; the chat list (titles, order, running) may have changed. */
export interface ChatTurnEvent {
  type: "chat.turn";
  projectId: string;
  chatId: string;
}

export interface CheckpointBlockedEvent {
  type: "checkpoint.blocked";
  projectId: string;
  reason: string;
}

/** Remount the preview iframe (agent ctl or turn-end). */
export interface PreviewReloadEvent {
  type: "preview.reload";
  projectId: string;
}

/** The dev server is about to reinstall the draft's packages (its deps changed). */
export interface PreviewDepsInstallingEvent {
  type: "preview.deps_installing";
  projectId: string;
}

/** Reload Content collections, entry lists, and the open entry (agent ctl or turn-end). */
export interface ContentRefreshEvent {
  type: "content.refresh";
  projectId: string;
}

/** A sync chat was opened or moved to a new phase. */
export interface SyncChangedEvent {
  type: "sync.changed";
  projectId: string;
  conflictSessionId: string;
  chatId: string;
  phase: SyncPhase;
}

/** The daemon finished a Get latest that had skipped the current change (agent went idle). */
export interface ChangesUpdatedEvent {
  type: "changes.updated";
  projectId: string;
  result?: ChangesUpdate;
  error?: string;
}

/** An assistant (or the site builder) moved through its download lifecycle. */
export interface SetupRuntimeEvent {
  type: "setup.runtime";
  /** `node`, or a provider id such as `opencode`. */
  runtime: string;
  phase: RuntimePhase;
  version?: string | null;
  source?: RuntimeSource | null;
  /** 0–100 while downloading; null otherwise. */
  progress?: number | null;
  message?: string | null;
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
  | CheckpointBlockedEvent
  | ChangesUpdatedEvent
  | PreviewReloadEvent
  | PreviewDepsInstallingEvent
  | ContentRefreshEvent
  | ChatTurnEvent
  | ChatPermissionEvent
  | SyncChangedEvent
  | SetupRuntimeEvent
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

/** Download lifecycle of an assistant (or the site builder), mirroring the daemon. */
export type RuntimePhase =
  | "notDownloaded"
  | "downloading"
  | "finishing"
  | "ready"
  | "downloadError"
  | "updateAvailable";

/** Where a ready copy came from: downloaded by orso, or already on the machine. */
export type RuntimeSource = "managed" | "system";

export interface RuntimeStatus {
  phase: RuntimePhase;
  version: string | null;
  source: RuntimeSource | null;
  /** 0–100 while downloading; null otherwise. */
  progress: number | null;
  /** A plain-language detail (error, "Setting up…"); null when there is none. */
  message: string | null;
  downloadBytes?: number;
  installedBytes?: number;
}

/** One assistant as `GET /api/setup/status` (and `/runtimes`) reports it. */
export interface SetupProvider {
  id: string;
  name: string;
  /** A runnable copy exists. Older daemons only report this; newer ones also send `runtime`. */
  available?: boolean;
  runtime?: RuntimeStatus;
  installed?: boolean;
  /** `true`, `false`, or `null` when the daemon could not tell. Never guessed. */
  signedIn?: boolean | null;
  /** Chat can start now: the download is ready and a model is reachable. */
  chatReady?: boolean;
  /** Works without an account (OpenCode). */
  freeModels?: boolean;
  /** Why chat is not ready, in plain language. */
  reason?: string | null;
}

/** `GET /api/setup/status`: which prerequisites this machine has. */
export interface SetupStatus {
  git: boolean;
  gh: boolean;
  githubSignedIn: boolean;
  /** The site builder's download state. */
  node?: RuntimeStatus;
  providers: SetupProvider[];
  projectsDir: string;
  /** Daemon-backed dismiss flag (survives ephemeral UI origins). */
  setupDismissed?: boolean;
}

/** `GET /api/setup/runtimes`: the assistant and site-builder download snapshot. */
export interface SetupSnapshot {
  node?: RuntimeStatus;
  providers: SetupProvider[];
}

/** `POST /api/setup/providers/{id}/sign-in` (and code/cancel) responses. */
export interface ProviderSignInState {
  state: "browser" | "code" | "connected" | "cancelled" | "failed";
  /** Where to finish signing in, when the flow needs a browser. */
  url?: string;
  /** The flow asked for a pasted code. */
  needsCode?: boolean;
  /** Plain-language detail for waiting states and failures. */
  message?: string;
}

/** `POST /api/setup/providers/{id}/verify`: a temp-session handshake succeeded. */
export interface ProviderVerifyResult {
  chatReady: boolean;
  reason?: string | null;
}

/** `GET /api/setup/runtimes`: the assistant and site-builder download snapshot. */
export interface SetupSnapshot {
  node?: RuntimeStatus;
  providers: SetupProvider[];
}

/** `POST /api/setup/providers/{id}/sign-in` (and code/cancel) responses. */
export interface ProviderSignInState {
  state: "browser" | "code" | "connected" | "cancelled" | "failed";
  /** Where to finish signing in, when the flow needs a browser. */
  url?: string;
  /** The flow asked for a pasted code. */
  needsCode?: boolean;
  /** Plain-language detail for waiting states and failures. */
  message?: string;
}

/** `POST /api/setup/providers/{id}/verify`: a temp-session handshake succeeded. */
export interface ProviderVerifyResult {
  chatReady: boolean;
  reason?: string | null;
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
  pushedAt: string | null;
}

export interface ContentCollection {
  name: string;
  loader: { base?: string; pattern?: string };
  kind: "json" | "markdown" | "mdx";
  singleton?: string | null;
  editable: boolean;
  /** False when the integration cannot edit this collection at all; `editable` also drops while the preview is down. */
  supportedForEditing?: boolean;
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
export interface ContentEntryCreate {
  project: string; collection: string; slug: string; branch: string;
  fields: Record<string, unknown>; body?: string;
}
export interface ContentEntryRename {
  project: string; collection: string; id: string; slug: string; branch: string; hash: string;
}
export interface ContentEntryDelete { project: string; collection: string; id: string; branch: string; hash: string }
/** A schema reference field in another entry that names the inspected entry. */
export interface ContentReference { collection: string; id: string; path: string; field: string }
/** A local Markdown link to the entry's public URL; reported, never rewritten. */
export interface ContentProseLink { collection: string; id: string; path: string; url: string; line: number }
/** Dry-run impact shown before a delete is confirmed. */
export interface ContentImpact { references: ContentReference[]; assets: string[]; proseLinks: ContentProseLink[] }
export interface ContentEntrySummary { collection: string; id: string; path: string; kind: "json" | "markdown" | "mdx" }
export interface ContentEntry extends ContentEntrySummary {
  source: string; fields: Record<string, unknown> | null; body: string; branch: string; hash: string;
}
/** An image sent with a save; the entry names it with a `orso-image:<clientId>` token. */
export interface ContentImageUpload { clientId: string; alt: string; filename: string; file: Blob }
/** A saved entry; a save with images also says where each one went, by client ID. */
export interface SavedContentEntry extends ContentEntry {
  images?: Record<string, { path: string; link: string }>;
}
/** The project's root `DESIGN.md` frontmatter; unknown keys are kept as-is. */
export type DesignTokens = Record<string, unknown>;
/** `exists: false` carries a `starter` to offer instead of a file. */
export interface DesignDoc {
  exists: boolean; tokens: DesignTokens; body: string; hash?: string; branch: string;
  starter?: { tokens: DesignTokens; body: string };
}
/** The fields of the repo-root `orso.json`; each one falls back to its default when unset. */
export interface ProjectSettings { root: string; installDir: string; install: string; dev: string }
/** `settings` is the file as written (unknown keys included); `errors` is keyed by field, `_file` when it is not a JSON object. */
export interface SettingsDoc {
  exists: boolean; settings: Record<string, unknown>; defaults: ProjectSettings; hash?: string; branch: string;
  errors: Record<string, string>;
}
