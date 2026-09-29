import type { DaemonConfig } from "./config";
import {
  ERROR_CODES,
  type Checkpoint,
  type ErrorCode,
  type FileContent,
  type Health,
  type PermissionDecision,
  type Project,
  type Provider,
  type Tree,
  type Turn,
} from "./types";

/** The daemon answered with an error (or something unparseable). */
export class ApiError extends Error {
  readonly code: ErrorCode | "UNKNOWN";
  readonly status: number;

  constructor(code: ErrorCode | "UNKNOWN", message: string, status: number) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.status = status;
  }

  get isUnauthorized(): boolean {
    return this.code === "UNAUTH" || this.status === 401;
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
        const { code, message } = err as { code?: unknown; message?: unknown };
        const text = typeof message === "string" && message ? message : statusText;
        return new ApiError(isErrorCode(code) ? code : "UNKNOWN", text || `HTTP ${status}`, status);
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

  async function send(method: string, path: string, opts: { query?: Query; body?: unknown } = {}) {
    const { origin, token } = cfg();
    const headers: Record<string, string> = {};
    if (token) headers.Authorization = `Bearer ${token}`;
    const init: RequestInit = { method, headers };
    if (opts.body !== undefined) {
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

  async function json<T>(method: string, path: string, opts?: { query?: Query; body?: unknown }) {
    const res = await send(method, path, opts);
    try {
      return (await res.json()) as T;
    } catch {
      throw new ApiError("UNKNOWN", "daemon returned invalid JSON", res.status);
    }
  }

  const enc = encodeURIComponent;

  return {
    health: () => json<Health>("GET", "/api/health"),

    listAgents: async (): Promise<Provider[]> =>
      (await json<{ providers: Provider[] }>("GET", "/api/agents")).providers,

    listProjects: async (): Promise<Project[]> =>
      (await json<{ projects: Project[] }>("GET", "/api/projects")).projects,
    createProject: (name: string) => json<Project>("POST", "/api/projects", { body: { name } }),
    updateProject: (id: string, patch: { autoApprove?: boolean; name?: string }) =>
      json<Project>("PATCH", `/api/projects/${enc(id)}`, { body: patch }),
    deleteProject: (id: string) =>
      json<{ deleted: string }>("DELETE", `/api/projects/${enc(id)}`, {
        query: { confirm: true },
      }),
    restartDevserver: (id: string) =>
      json<unknown>("POST", `/api/projects/${enc(id)}/devserver/restart`),

    startSession: async (project: string, provider: string): Promise<string> =>
      (
        await json<{ sessionId: string }>("POST", "/api/agent/session", {
          body: { project, provider },
        })
      ).sessionId,
    prompt: async (project: string, sessionId: string, text: string): Promise<string> =>
      (
        await json<{ turnId: string }>("POST", "/api/agent/prompt", {
          body: { project, sessionId, text },
        })
      ).turnId,
    cancelTurn: (turnId: string) =>
      json<unknown>("POST", `/api/agent/prompt/${enc(turnId)}/cancel`),
    resolvePermission: (requestId: string, decision: PermissionDecision) =>
      json<unknown>("POST", `/api/agent/permission/${enc(requestId)}`, { body: { decision } }),
    history: async (project: string, session?: string): Promise<Turn[]> =>
      (
        await json<{ turns: Turn[] }>("GET", "/api/agent/history", {
          query: { project, session },
        })
      ).turns,

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

    /** URL to embed the live preview in an iframe. Auth is via the `wb_session` cookie. */
    previewUrl: (id: string) => `${cfg().origin}/preview/${enc(id)}/`,
  };
}
