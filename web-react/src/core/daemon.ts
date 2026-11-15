import { createApi, type Api } from "./api";
import {
  browserResolveDaemonConfig,
  clearDaemonConfig,
  parseDaemonLink,
  saveDaemonConfig,
  type DaemonConfig,
} from "./config";
import { buildEventsUrl, createEventSocket, type EventSocketOptions } from "./socket";
import type { ConnectionStatus, DaemonEvent, DaemonEventType } from "./types";

export type UnreachableReason = "network" | "rejected";

type Handler<T extends DaemonEventType> = (event: Extract<DaemonEvent, { type: T }>) => void;

export interface DaemonDeps {
  config: DaemonConfig;
  persist?: (config: DaemonConfig | null) => void;
  socketOptions?: Partial<EventSocketOptions>;
  api?: Api;
}

export interface DaemonSnapshot {
  config: DaemonConfig;
  status: ConnectionStatus;
  reason: UnreachableReason | null;
}

/**
 * Framework-agnostic daemon connection (REST + WS). React subscribes via useSyncExternalStore.
 */
export function createDaemon(deps: DaemonDeps) {
  let config = deps.config;
  let status: ConnectionStatus = "connecting";
  let reason: UnreachableReason | null = null;
  let snapshot: DaemonSnapshot = { config, status, reason };
  const listeners = new Set<() => void>();
  const api = deps.api ?? createApi(() => config);
  const handlers = new Map<DaemonEventType, Set<(e: never) => void>>();
  const reconnectHandlers = new Set<() => void>();

  function refreshSnapshot() {
    if (
      snapshot.config === config &&
      snapshot.status === status &&
      snapshot.reason === reason
    ) {
      return false;
    }
    snapshot = { config, status, reason };
    return true;
  }

  function emit() {
    if (!refreshSnapshot()) return;
    for (const l of listeners) l();
  }

  async function probe() {
    try {
      await api.health();
      reason = "rejected";
    } catch {
      reason = "network";
    }
    emit();
  }

  const socket = createEventSocket({
    ...deps.socketOptions,
    url: () => buildEventsUrl(config),
    onEvent(event) {
      for (const h of handlers.get(event.type) ?? []) (h as (e: DaemonEvent) => void)(event);
    },
    onStatus(next) {
      status = next;
      if (next === "unreachable") void probe();
      else reason = null;
      emit();
    },
    onReconnected() {
      for (const h of reconnectHandlers) h();
    },
  });

  return {
    api,
    /** Cached — must return the same reference until status/config/reason change. */
    getSnapshot(): DaemonSnapshot {
      return snapshot;
    },
    subscribe(listener: () => void): () => void {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    start: () => socket.connect(),
    stop: () => socket.close(),
    retry: () => socket.retryNow(),
    on<T extends DaemonEventType>(type: T, handler: Handler<T>): () => void {
      let set = handlers.get(type);
      if (!set) handlers.set(type, (set = new Set()));
      set.add(handler as (e: never) => void);
      return () => set.delete(handler as (e: never) => void);
    },
    onReconnected(handler: () => void): () => void {
      reconnectHandlers.add(handler);
      return () => reconnectHandlers.delete(handler);
    },
    connectTo(link: string): boolean {
      const next = parseDaemonLink(link);
      if (!next) return false;
      config = next;
      deps.persist?.(next);
      emit();
      socket.retryNow();
      return true;
    },
    forget() {
      deps.persist?.(null);
    },
  };
}

export type Daemon = ReturnType<typeof createDaemon>;

let instance: Daemon | null = null;

export function getDaemon(): Daemon {
  if (!instance) {
    instance = createDaemon({
      config: browserResolveDaemonConfig(),
      persist: (c) => {
        if (c) saveDaemonConfig(window.localStorage, c, window.location.origin);
        else clearDaemonConfig(window.localStorage);
      },
    });
  }
  return instance;
}

/** Test helper: reset the singleton between suites. */
export function resetDaemonForTests() {
  instance?.stop();
  instance = null;
}
