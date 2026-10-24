import type { DaemonConfig } from "./config";
import type { ConnectionStatus, DaemonEvent, DaemonEventType } from "./types";

// ---------------------------------------------------------------------------
// URL + event decoding
// ---------------------------------------------------------------------------

/** `http(s)://host` → `ws(s)://host/ws/events[?t=token]`. */
export function buildEventsUrl(config: DaemonConfig): string {
  const url = new URL("/ws/events", config.origin);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  if (config.token) url.searchParams.set("t", config.token);
  return url.toString();
}

type Shape = Record<string, "string" | "number" | "nullable-string" | "nullable-number">;

/** Required fields per event type (beyond `type`); enough to protect consumers from junk. */
const SHAPES: Record<DaemonEventType, Shape> = {
  "devserver.status": { projectId: "string", state: "string" },
  "checkpoint.created": { projectId: "string", sha: "string", summary: "string" },
  "fs.change": { projectId: "string", path: "string", kind: "string" },
  "checkpoint.blocked": { projectId: "string", reason: "string" },
  "changes.updated": { projectId: "string" },
  "changes.auto_resolve": { projectId: "string", sessionId: "string", state: "string" },
  "preview.reload": { projectId: "string" },
  "preview.deps_installing": { projectId: "string" },
  "content.refresh": { projectId: "string" },
  "chat.turn": { projectId: "string", chatId: "string" },
  "events.lagged": { missed: "number" },
};

function matches(obj: Record<string, unknown>, shape: Shape): boolean {
  return Object.entries(shape).every(([key, kind]) => {
    const v = obj[key];
    if (kind === "nullable-string") return v === undefined || v === null || typeof v === "string";
    if (kind === "nullable-number") return v === null || typeof v === "number";
    return typeof v === kind;
  });
}

/**
 * Decode one WebSocket text frame. Returns `null` for anything that is not a known,
 * well-formed event (unknown types are ignored so newer daemons don't break older UIs).
 */
export function decodeEvent(raw: unknown): DaemonEvent | null {
  let value: unknown = raw;
  if (typeof raw === "string") {
    try {
      value = JSON.parse(raw);
    } catch {
      return null;
    }
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const obj = value as Record<string, unknown>;
  const type = obj.type;
  if (typeof type !== "string" || !Object.hasOwn(SHAPES, type)) return null;
  if (!matches(obj, SHAPES[type as DaemonEventType])) return null;
  return obj as unknown as DaemonEvent;
}

// ---------------------------------------------------------------------------
// Reconnecting socket
// ---------------------------------------------------------------------------

/** The subset of the WebSocket API we use (so tests can inject a fake). */
export interface SocketLike {
  onopen: ((ev: unknown) => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  onclose: ((ev: unknown) => void) | null;
  onerror: ((ev: unknown) => void) | null;
  close(): void;
}
export type SocketFactory = (url: string) => SocketLike;

export interface EventSocketOptions {
  /** Evaluated on every (re)connect so origin/token changes are picked up. */
  url: () => string;
  onEvent: (event: DaemonEvent) => void;
  onStatus?: (status: ConnectionStatus) => void;
  /** Fired when a connection opens after a previous one was lost: refetch state here. */
  onReconnected?: () => void;
  createSocket?: SocketFactory;
  /** Delay before the first retry; doubles up to `maxDelayMs`. */
  initialDelayMs?: number;
  maxDelayMs?: number;
  /** Consecutive failed attempts before status becomes `unreachable`. */
  unreachableAfter?: number;
  setTimer?: (fn: () => void, ms: number) => unknown;
  clearTimer?: (handle: unknown) => void;
}

export interface EventSocket {
  connect(): void;
  /** Drop any backoff and reconnect immediately (the "Retry" button). */
  retryNow(): void;
  close(): void;
  readonly status: ConnectionStatus;
}

export function createEventSocket(opts: EventSocketOptions): EventSocket {
  const createSocket: SocketFactory =
    opts.createSocket ?? ((url) => new WebSocket(url) as unknown as SocketLike);
  const initialDelay = opts.initialDelayMs ?? 500;
  const maxDelay = opts.maxDelayMs ?? 10_000;
  const unreachableAfter = opts.unreachableAfter ?? 3;
  const setTimer = opts.setTimer ?? ((fn, ms) => setTimeout(fn, ms));
  const clearTimer =
    opts.clearTimer ?? ((h) => clearTimeout(h as ReturnType<typeof setTimeout>));

  let status: ConnectionStatus = "connecting";
  let socket: SocketLike | null = null;
  let timer: unknown = null;
  let failures = 0;
  let delay = initialDelay;
  let everOpened = false;
  let closed = true;

  function setStatus(next: ConnectionStatus) {
    if (next === status) return;
    status = next;
    opts.onStatus?.(next);
  }

  function detach(s: SocketLike) {
    s.onopen = s.onmessage = s.onclose = s.onerror = null;
  }

  function clearRetry() {
    if (timer !== null) {
      clearTimer(timer);
      timer = null;
    }
  }

  function open() {
    clearRetry();
    if (socket) {
      detach(socket);
      socket.close();
      socket = null;
    }
    let s: SocketLike;
    try {
      s = createSocket(opts.url());
    } catch {
      handleLoss();
      return;
    }
    socket = s;
    s.onopen = () => {
      if (socket !== s) return;
      const reconnected = everOpened;
      everOpened = true;
      failures = 0;
      delay = initialDelay;
      setStatus("open");
      if (reconnected) opts.onReconnected?.();
    };
    s.onmessage = (ev) => {
      if (socket !== s) return;
      const decoded = decodeEvent(ev.data);
      if (decoded) opts.onEvent(decoded);
    };
    const lost = () => {
      if (socket !== s) return;
      detach(s);
      socket = null;
      handleLoss();
    };
    s.onclose = lost;
    s.onerror = lost;
  }

  function handleLoss() {
    if (closed) return;
    failures += 1;
    setStatus(failures >= unreachableAfter ? "unreachable" : everOpened ? "reconnecting" : "connecting");
    clearRetry();
    timer = setTimer(() => {
      timer = null;
      if (!closed) open();
    }, delay);
    delay = Math.min(delay * 2, maxDelay);
  }

  return {
    get status() {
      return status;
    },
    connect() {
      if (!closed) return;
      closed = false;
      failures = 0;
      delay = initialDelay;
      setStatus("connecting");
      open();
    },
    retryNow() {
      closed = false;
      failures = 0;
      delay = initialDelay;
      setStatus(everOpened ? "reconnecting" : "connecting");
      open();
    },
    close() {
      closed = true;
      clearRetry();
      if (socket) {
        detach(socket);
        socket.close();
        socket = null;
      }
    },
  };
}
