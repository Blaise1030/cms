// Daemon-origin and token handling. Ported from web/src/core/config.ts for the Lenso spike.

export interface DaemonConfig {
  /** Daemon origin without a trailing slash, e.g. `http://127.0.0.1:7800`. */
  origin: string;
  /** Bearer / `?t=` token. Absent when relying on the `wb_session` cookie or `--no-auth`. */
  token?: string;
}

/** What we persist: `origin` is only stored when it differs from the page origin. */
interface StoredConfig {
  origin?: string;
  token?: string;
}

export const STORAGE_KEY = "orso.daemon";

export function normalizeOrigin(input: string): string {
  const url = new URL(input.trim());
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error(`unsupported protocol: ${url.protocol}`);
  }
  return url.origin;
}

export function isCrossSite(pageOrigin: string, daemonOrigin: string): boolean {
  try {
    const page = new URL(pageOrigin);
    const daemon = new URL(daemonOrigin);
    return page.protocol !== daemon.protocol || page.hostname !== daemon.hostname;
  } catch {
    return false;
  }
}

export function parseDaemonLink(link: string): DaemonConfig | null {
  try {
    const url = new URL(link.trim());
    const origin = normalizeOrigin(url.href);
    const token = url.searchParams.get("t") || undefined;
    return token ? { origin, token } : { origin };
  } catch {
    return null;
  }
}

function readStored(storage: Pick<Storage, "getItem">): StoredConfig {
  try {
    const raw = storage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const v: unknown = JSON.parse(raw);
    if (typeof v !== "object" || v === null) return {};
    const { origin, token } = v as Record<string, unknown>;
    return {
      origin: typeof origin === "string" ? origin : undefined,
      token: typeof token === "string" ? token : undefined,
    };
  } catch {
    return {};
  }
}

export function saveDaemonConfig(
  storage: Pick<Storage, "setItem">,
  config: DaemonConfig,
  pageOrigin: string,
): void {
  const stored: StoredConfig = {
    origin: config.origin === pageOrigin ? undefined : config.origin,
    token: config.token,
  };
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(stored));
  } catch {
    // Storage may be unavailable (private mode); the in-memory config still works.
  }
}

export function clearDaemonConfig(storage: Pick<Storage, "removeItem">): void {
  try {
    storage.removeItem(STORAGE_KEY);
  } catch {
    // ignore
  }
}

export interface ResolveDeps {
  location: Pick<Location, "origin" | "search" | "pathname" | "hash">;
  storage: Pick<Storage, "getItem" | "setItem">;
  /** Called with the cleaned URL when `?t=` / `?daemon=` were consumed. */
  replaceUrl?: (url: string) => void;
}

/**
 * Resolve the daemon config:
 *  1. `?daemon=<origin>` and/or `?t=<token>` in the page URL (consumed and stripped),
 *  2. otherwise the stored config,
 *  3. otherwise the page origin (SPA embedded in the daemon / Vite proxy).
 */
export function resolveDaemonConfig(deps: ResolveDeps): DaemonConfig {
  const { location, storage } = deps;
  const params = new URLSearchParams(location.search);
  const daemonParam = params.get("daemon");
  const tokenParam = params.get("t");
  const stored = readStored(storage);

  let origin = stored.origin ?? location.origin;
  let token = stored.token;
  let consumed = false;

  if (daemonParam) {
    try {
      origin = normalizeOrigin(daemonParam);
      if (!tokenParam) token = undefined;
    } catch {
      // ignore an unparseable ?daemon=; keep the previous origin
    }
    consumed = true;
  }
  if (tokenParam) {
    token = tokenParam;
    consumed = true;
  }

  const config: DaemonConfig = token ? { origin, token } : { origin };
  if (consumed) {
    saveDaemonConfig(storage, config, location.origin);
    params.delete("daemon");
    params.delete("t");
    const rest = params.toString();
    deps.replaceUrl?.(`${location.pathname}${rest ? `?${rest}` : ""}${location.hash}`);
  }
  return config;
}

export function browserResolveDaemonConfig(): DaemonConfig {
  return resolveDaemonConfig({
    location: window.location,
    storage: window.localStorage,
    replaceUrl: (url) => window.history.replaceState(null, "", url),
  });
}
