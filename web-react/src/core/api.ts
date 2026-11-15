import type { DaemonConfig } from "./config";
import { ERROR_CODES, type ErrorCode, type Health, type Project } from "./types";

export interface ApiErrorDetail {
  path: (string | number)[];
  message: string;
}

export class ApiError extends Error {
  readonly code: ErrorCode | "UNKNOWN";
  readonly status: number;
  readonly reason?: string;
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
}

export class NetworkError extends Error {
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = "NetworkError";
  }
}

function isErrorCode(v: unknown): v is ErrorCode {
  return typeof v === "string" && (ERROR_CODES as readonly string[]).includes(v);
}

export function mapErrorResponse(status: number, statusText: string, body: string): ApiError {
  try {
    const parsed: unknown = JSON.parse(body);
    if (typeof parsed === "object" && parsed !== null && "error" in parsed) {
      const err = (parsed as { error: unknown }).error;
      if (typeof err === "object" && err !== null) {
        const { code, message, reason, details } = err as {
          code?: unknown;
          message?: unknown;
          reason?: unknown;
          details?: unknown;
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

export type Api = ReturnType<typeof createApi>;

/** Spike REST client: health + projects only. */
export function createApi(
  config: DaemonConfig | (() => DaemonConfig),
  fetchImpl: FetchLike = (input, init) => fetch(input, init),
) {
  const cfg = typeof config === "function" ? config : () => config;

  function authHeaders(): Record<string, string> {
    const { token } = cfg();
    return token ? { Authorization: `Bearer ${token}` } : {};
  }

  async function send(method: string, path: string, body?: unknown) {
    const { origin } = cfg();
    const headers = authHeaders();
    const init: RequestInit = { method, headers };
    if (body !== undefined) {
      headers["Content-Type"] = "application/json";
      init.body = JSON.stringify(body);
    }
    let res: Response;
    try {
      res = await fetchImpl(`${origin}${path}`, init);
    } catch (cause) {
      throw new NetworkError(`cannot reach daemon at ${origin}`, { cause });
    }
    if (!res.ok) {
      throw mapErrorResponse(res.status, res.statusText, await res.text().catch(() => ""));
    }
    return res;
  }

  async function json<T>(method: string, path: string, body?: unknown) {
    const res = await send(method, path, body);
    try {
      return (await res.json()) as T;
    } catch {
      throw new ApiError("UNKNOWN", "daemon returned invalid JSON", res.status);
    }
  }

  return {
    health: () => json<Health>("GET", "/api/health"),
    listProjects: async (): Promise<Project[]> =>
      (await json<{ projects: Project[] }>("GET", "/api/projects")).projects,
  };
}
