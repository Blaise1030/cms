import type { UIMessage } from "ai";
import type { DaemonConfig } from "./config";
import {
  ERROR_CODES,
  type ChatSummary,
  type ContentCollection,
  type ContentEntrySummary,
  type ContentEntry,
  type ContentEntryUpdate,
  type ContentEntryCreate,
  type ContentImageUpload,
  type SavedContentEntry,
  type ContentEntryRename,
  type ContentEntryDelete,
  type ContentImpact,
  type ContentVersion,
  type DesignDoc,
  type DesignTokens,
  type FileVersion,
  type Change,
  type ChangesUpdate,
  type AutoResolveStatus,
  type ConflictCandidate,
  type ConflictChoice,
  type ConflictSession,
  type ConflictPreviews,
  type SharedChange,
  type PrStatus,
  type Checkpoint,
  type ErrorCode,
  type DeviceStart,
  type DevicePoll,
  type FileContent,
  type GithubRepo,
  type GithubStatus,
  type Health,
  type Project,
  type SetupStatus,
  type Tree,
} from "./types";

export interface ApiErrorDetail { path: (string | number)[]; message: string }

/** The daemon answered with an error (or something unparseable). */
export class ApiError extends Error {
  readonly code: ErrorCode | "UNKNOWN";
  readonly status: number;
  /** Machine-readable sub-code (e.g. `stale_content`), when the daemon gave one. */
  readonly reason?: string;
  /** Per-field validation problems, when the daemon gave them. */
  readonly details?: ApiErrorDetail[];

  constructor(
    code: ErrorCode | "UNKNOWN",
    message: string,
    status: number,
    extra: { reason?: string; details?: ApiErrorDetail[] } = {},
  ) {
    super(message);
    this.reason = extra.reason;
    this.details = extra.details;
    this.name = "ApiError";
    this.code = code;
    this.status = status;
  }

  get isUnauthorized(): boolean {
    return this.code === "UNAUTH" || this.status === 401;
  }

  /** GitHub needs (re-)sign-in: an expired session, or a call that needs this app's own sign-in. */
  get needsGithubSignIn(): boolean {
    return this.isUnauthorized || this.reason === "github_sign_in";
  }
}

/** The daemon could not be reached at all (fetch rejected). */
export class NetworkError extends Error {
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = "NetworkError";
  }
}

function isErrorCode(v: unknown): v is ErrorCode {
  return typeof v === "string" && (ERROR_CODES as readonly string[]).includes(v);
}

/**
 * Map a failed response to an `ApiError`. Understands the daemon envelope
 * `{ error: { code, message } }`; anything else (proxy HTML, empty body) becomes
 * `UNKNOWN` with a status-derived message.
 */
export function mapErrorResponse(status: number, statusText: string, body: string): ApiError {
  try {
    const parsed: unknown = JSON.parse(body);
    if (typeof parsed === "object" && parsed !== null && "error" in parsed) {
      const err = (parsed as { error: unknown }).error;
      if (typeof err === "object" && err !== null) {
        const { code, message, reason, details } = err as {
          code?: unknown; message?: unknown; reason?: unknown; details?: unknown;
        };
        const text = typeof message === "string" && message ? message : statusText;
        return new ApiError(isErrorCode(code) ? code : "UNKNOWN", text || `HTTP ${status}`, status, {
          reason: typeof reason === "string" ? reason : undefined,
          details: Array.isArray(details) ? (details as ApiErrorDetail[]) : undefined,
        });
      }
    }
  } catch {
    // not JSON
  }
  return new ApiError("UNKNOWN", statusText || `HTTP ${status}`, status);
}

export type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

type Query = Record<string, string | boolean | undefined>;

function queryString(q?: Query): string {
  if (!q) return "";
  const sp = new URLSearchParams();
  for (const [k, v] of Object.entries(q)) {
    if (v !== undefined) sp.set(k, String(v));
  }
  const s = sp.toString();
  return s ? `?${s}` : "";
}

export type Api = ReturnType<typeof createApi>;

/**
 * Typed REST client. `config` may be a getter so the origin/token can change at runtime
 * (e.g. user pastes a new daemon link) without rebuilding the client.
 */
export function createApi(
  config: DaemonConfig | (() => DaemonConfig),
  fetchImpl: FetchLike = (input, init) => fetch(input, init),
) {
  const cfg = typeof config === "function" ? config : () => config;

  type SendOpts = { query?: Query; body?: unknown; form?: FormData; blob?: Blob };

  function authHeaders(): Record<string, string> {
    const { token } = cfg();
    return token ? { Authorization: `Bearer ${token}` } : {};
  }

  async function send(method: string, path: string, opts: SendOpts = {}) {
    const { origin } = cfg();
    const headers = authHeaders();
    const init: RequestInit = { method, headers };
    // The browser sets the multipart Content-Type with its boundary.
    if (opts.form) init.body = opts.form;
    else if (opts.blob) {
      headers["Content-Type"] = opts.blob.type;
      init.body = opts.blob;
    } else if (opts.body !== undefined) {
      headers["Content-Type"] = "application/json";
      init.body = JSON.stringify(opts.body);
    }
    let res: Response;
    try {
      res = await fetchImpl(`${origin}${path}${queryString(opts.query)}`, init);
    } catch (cause) {
      throw new NetworkError(`cannot reach daemon at ${origin}`, { cause });
    }
    if (!res.ok) {
      throw mapErrorResponse(res.status, res.statusText, await res.text().catch(() => ""));
    }
    return res;
  }

  async function json<T>(method: string, path: string, opts?: SendOpts) {
    const res = await send(method, path, opts);
    try {
      return (await res.json()) as T;
    } catch {
      throw new ApiError("UNKNOWN", "daemon returned invalid JSON", res.status);
    }
  }

  const enc = encodeURIComponent;

  return {
    listContentCollections: async (project: string): Promise<ContentCollection[]> =>
      (await json<{collections: ContentCollection[]}>("GET", "/api/content/collections", {query:{project}})).collections,
    listContentEntries: async (project: string, collection: string): Promise<ContentEntrySummary[]> =>
      (await json<{entries: ContentEntrySummary[]}>("GET", "/api/content/entries", {query:{project,collection}})).entries,
    contentEntry: (project: string, collection: string, id: string) =>
      json<ContentEntry>("GET", "/api/content/entry", {query:{project,collection,id}}),
    contentHistory: async (project: string, collection: string, id: string): Promise<FileVersion[]> =>
      (await json<{versions: FileVersion[]}>("GET", "/api/content/history", {query:{project,collection,id}})).versions,
    /** 404 when `sha` is no longer in the entry's history. */
    contentVersion: (project: string, collection: string, id: string, sha: string) =>
      json<ContentVersion>("GET", `/api/content/history/${enc(sha)}`, {query:{project,collection,id}}),
    /** JSON without images; with images, multipart: a JSON `entry` part plus one part per image named by its client ID. */
    updateContentEntry: (body: ContentEntryUpdate, images: ContentImageUpload[] = []) => {
      if (!images.length) return json<SavedContentEntry>("PUT", "/api/content/entry", { body });
      const form = new FormData();
      const meta = images.map(({ clientId, alt, filename }) => ({ clientId, alt, filename }));
      form.append("entry", new Blob([JSON.stringify({ ...body, images: meta })], { type: "application/json" }));
      for (const image of images) form.append(image.clientId, image.file, image.filename);
      return json<SavedContentEntry>("PUT", "/api/content/entry", { form });
    },
    /** Create an entry on the active change with complete starting fields. */
    createContentEntry: (body: ContentEntryCreate) =>
      json<SavedContentEntry>("POST", "/api/content/entry", { body }),
    /** A project file's bytes, served read-only (the session cookie authenticates it, like Preview). */
    rawFileUrl: (project: string, path: string) =>
      `${cfg().origin}/api/file?project=${enc(project)}&path=${enc(path)}&raw=true`,
    contentImpact: (project: string, collection: string, id: string) =>
      json<ContentImpact>("POST", "/api/content/entry/impact", { body: { project, collection, id } }),
    renameContentEntry: (body: ContentEntryRename) =>
      json<{ entry: ContentEntry; entries: ContentEntrySummary[] }>("POST", "/api/content/entry/rename", { body }),
    deleteContentEntry: (body: ContentEntryDelete) =>
      json<{ entries: ContentEntrySummary[] }>("DELETE", "/api/content/entry", { body }),
    design: (project: string) => json<DesignDoc>("GET", "/api/design", { query: { project } }),
    /** Omit `hash` to create the file; the daemon refuses a stale hash or changed branch with 409. */
    saveDesign: (body: { project: string; branch: string; hash?: string; tokens: DesignTokens; body: string }) =>
      json<DesignDoc>("PUT", "/api/design", { body }),
    health: () => json<Health>("GET", "/api/health"),

    listProjects: async (): Promise<Project[]> =>
      (await json<{ projects: Project[] }>("GET", "/api/projects")).projects,
    createProject: (name: string) => json<Project>("POST", "/api/projects", { body: { name } }),
    updateProject: (id: string, patch: { autoApprove?: boolean; name?: string }) =>
      json<Project>("PATCH", `/api/projects/${enc(id)}`, { body: patch }),
    deleteProject: (id: string) =>
      json<{ deleted: string }>("DELETE", `/api/projects/${enc(id)}`, {
        query: { confirm: true },
      }),
    cloneProject: (repo: string, name?: string) =>
      json<Project>("POST", "/api/github/clone", { body: name ? { repo, name } : { repo } }),
    githubStatus: () => json<GithubStatus>("GET", "/api/github/status"),
    setupStatus: () => json<SetupStatus>("GET", "/api/setup/status"),
    githubDeviceStart: () => json<DeviceStart>("POST", "/api/github/device"),
    githubDevicePoll: () => json<DevicePoll>("POST", "/api/github/device/poll"),
    githubSignOut: () => json<{ signedOut: true }>("POST", "/api/github/signout"),
    githubRepos: async (): Promise<GithubRepo[]> =>
      (await json<{ repos: GithubRepo[] }>("GET", "/api/github/repos")).repos,
    dismissProtectionHint: (id: string) =>
      json<{ dismissed: true }>("POST", `/api/projects/${enc(id)}/protection-hint/dismiss`),
    restartDevserver: (id: string) =>
      json<unknown>("POST", `/api/projects/${enc(id)}/devserver/restart`),

    terminalProviders: async (): Promise<{ id: string; command: string[] }[]> =>
      (
        await json<{ providers: { id: string; command: string[] }[] }>(
          "GET",
          "/api/terminal/providers",
        )
      ).providers,
    stopTerminal: (project: string) =>
      json<{ stopped: true }>("DELETE", `/api/terminal/${enc(project)}`),
    terminalStatus: (project: string) =>
      json<{ running: boolean; provider?: string; sessionId?: string | null; busy?: boolean }>(
        "GET",
        `/api/terminal/${enc(project)}`,
      ),
    /** Save an image for the agent; resolves to an absolute path on the daemon's machine. */
    attachTerminalImage: async (project: string, image: Blob): Promise<string> =>
      (await json<{ path: string }>("POST", `/api/terminal/${enc(project)}/attachments`, { blob: image })).path,
    listChats: (project: string) =>
      json<{ chats: ChatSummary[]; running: string | null }>("GET", `/api/chat/${enc(project)}`),
    chatHistory: (project: string, chatId: string) =>
      json<{ messages: UIMessage[]; provider: string; running: boolean }>(
        "GET",
        `/api/chat/${enc(project)}/${enc(chatId)}`,
      ),
    /** Stop the project's running chat turn, if any. */
    stopChat: (project: string) =>
      json<{ stopped: boolean }>("DELETE", `/api/chat/${enc(project)}/turn`),
    /** The chat stream endpoint; the AI SDK transport fetches it (and `/{chatId}/stream`) itself. */
    chatUrl: (project: string) => `${cfg().origin}/api/chat/${enc(project)}`,
    /** Headers for requests made outside this client (the chat transport). */
    authHeaders,
    createChange: (project: string, title: string) =>
      json<Change>("POST", "/api/changes", { body: { project, title } }),
    listChanges: async (project: string): Promise<Change[]> =>
      (await json<{ changes: Change[] }>("GET", "/api/changes", { query: { project } })).changes,
    activeChange: async (project: string): Promise<Change> =>
      json<Change>("GET", "/api/changes/active", { query: { project } }),
    switchChange: (project: string, branch: string) =>
      json<Change>("POST", `/api/changes/${enc(branch)}/switch`, { body: { project } }),
    updateChanges: (project: string) =>
      json<ChangesUpdate>("POST", "/api/changes/update", { body: { project } }),
    listConflicts: async (project: string): Promise<ConflictSession[]> =>
      (await json<{ sessions: ConflictSession[] }>("GET", "/api/changes/conflicts", { query: { project } })).sessions,
    getConflict: (project: string, id: string) =>
      json<ConflictSession>("GET", `/api/changes/conflicts/${enc(id)}`, { query: { project } }),
    conflictPreviews: (project: string, id: string) =>
      json<ConflictPreviews>("GET", `/api/changes/conflicts/${enc(id)}/previews`, { query: { project } }),
    chooseConflict: (project: string, id: string, path: string, choice: ConflictChoice) =>
      json<ConflictCandidate>("POST", `/api/changes/conflicts/${enc(id)}/choice`, { body: { project, path, choice } }),
    finishConflict: (project: string, id: string) =>
      json<{ commit: string }>("POST", `/api/changes/conflicts/${enc(id)}/finish`, { body: { project } }),
    abandonConflict: (project: string, id: string) =>
      json<{ abandoned: true }>("POST", `/api/changes/conflicts/${enc(id)}/abandon`, { body: { project } }),
    startAutoResolve: (project: string, sessionId: string, provider: string) =>
      json<{ started: true }>("POST", `/api/changes/conflicts/${enc(sessionId)}/auto`, { body: { project, provider } }),
    autoResolveStatus: (project: string, sessionId: string) =>
      json<AutoResolveStatus>("GET", `/api/changes/conflicts/${enc(sessionId)}/auto`, { query: { project } }),
    answerAutoResolve: (project: string, sessionId: string, answers: Record<string, string>) =>
      json<{ ok: true }>("POST", `/api/changes/conflicts/${enc(sessionId)}/auto/answers`, { body: { project, answers } }),
    cancelAutoResolve: (project: string, sessionId: string) =>
      json<{ ok: true }>("POST", `/api/changes/conflicts/${enc(sessionId)}/auto/cancel`, { body: { project } }),
    shareChange: (project: string, branch: string) =>
      json<SharedChange>("POST", `/api/changes/${enc(branch)}/share`, {
        body: { project },
      }),
    /** Delete a merged change's local branch and return to an up-to-date `main`. */
    cleanupChange: (project: string, branch: string) =>
      json<{ branch: string; mainUpdated?: boolean }>("POST", `/api/changes/${enc(branch)}/cleanup`, {
        body: { project },
      }),
    /** PR state / checks / preview URL of a change; null when it has no PR. */
    changeStatus: async (project: string, branch: string): Promise<PrStatus | null> =>
      (
        await json<{ pr: PrStatus | null }>("GET", `/api/changes/${enc(branch)}/status`, {
          query: { project },
        })
      ).pr,

    /** Mark a change's draft PR ready for review; returns its fresh status. */
    markChangeReady: async (project: string, branch: string): Promise<PrStatus | null> =>
      (
        await json<{ pr: PrStatus | null }>("POST", `/api/changes/${enc(branch)}/ready`, {
          body: { project },
        })
      ).pr,

    listCheckpoints: async (project: string): Promise<Checkpoint[]> =>
      (await json<{ checkpoints: Checkpoint[] }>("GET", "/api/checkpoints", { query: { project } }))
        .checkpoints,
    checkpointDiff: async (project: string, sha: string): Promise<string> =>
      (await send("GET", `/api/checkpoints/${enc(sha)}/diff`, { query: { project } })).text(),
    restoreCheckpoint: (project: string, sha: string) =>
      json<{ restored: string }>("POST", `/api/checkpoints/${enc(sha)}/restore`, {
        body: { project },
      }),

    tree: (project: string, path = "") => json<Tree>("GET", "/api/tree", { query: { project, path } }),
    file: (project: string, path: string) =>
      json<FileContent>("GET", "/api/file", { query: { project, path } }),

    /**
     * URL to embed the live preview in an iframe. Auth is via the `wb_session` cookie only (an
     * iframe cannot send `Authorization`), and the daemon mints it `HttpOnly; SameSite=Lax`.
     *
     * KNOWN LIMITATION: when the page and the daemon are cross-site (hosted PWA at
     * `app.<domain>` driving a loopback daemon) the browser neither stores nor sends that cookie
     * for the framed request, so every request inside the frame 401s. Only the embedded topology
     * (SPA served by the daemon, same origin) and same-site setups (e.g. `localhost:5173` ->
     * `localhost:7800`) work. `PreviewPanel` shows a notice instead of a broken frame in that case
     * (see `isCrossSite`); a real fix needs a relay or a short-lived URL-scoped preview token.
     */
    previewUrl: (id: string) => `${cfg().origin}/preview/${enc(id)}/`,
  };
}
