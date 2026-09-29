import { readonly, ref, shallowRef } from "vue";
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

/** Why the daemon is unreachable: nothing listening vs. listening but refusing us. */
export type UnreachableReason = "network" | "rejected";

type Handler<T extends DaemonEventType> = (event: Extract<DaemonEvent, { type: T }>) => void;

export interface DaemonDeps {
  config: DaemonConfig;
  /** Persist a config change (e.g. pasted daemon link). */
  persist?: (config: DaemonConfig | null) => void;
  socketOptions?: Partial<EventSocketOptions>;
  api?: Api;
}

/**
 * The single daemon connection: REST client + live event socket + reactive status.
 * Modules use it through `useDaemon()`.
 */
export function createDaemon(deps: DaemonDeps) {
  const config = shallowRef<DaemonConfig>(deps.config);
  const status = ref<ConnectionStatus>("connecting");
  const reason = ref<UnreachableReason | null>(null);
  const api = deps.api ?? createApi(() => config.value);
  const handlers = new Map<DaemonEventType, Set<(e: never) => void>>();
  const reconnectHandlers = new Set<() => void>();

  async function probe() {
    try {
      await api.health();
      // Daemon answered but the socket cannot connect: token or origin was refused.
      reason.value = "rejected";
    } catch {
      reason.value = "network";
    }
  }

  const socket = createEventSocket({
    ...deps.socketOptions,
    url: () => buildEventsUrl(config.value),
    onEvent(event) {
      for (const h of handlers.get(event.type) ?? []) (h as (e: DaemonEvent) => void)(event);
    },
    onStatus(next) {
      status.value = next;
      if (next === "unreachable") void probe();
      else reason.value = null;
    },
    onReconnected() {
      for (const h of reconnectHandlers) h();
    },
  });

  return {
    api,
    config: readonly(config),
    status: readonly(status),
    reason: readonly(reason),
    start: () => socket.connect(),
    stop: () => socket.close(),
    retry: () => socket.retryNow(),
    /** Subscribe to one event type. Returns an unsubscribe function. */
    on<T extends DaemonEventType>(type: T, handler: Handler<T>): () => void {
      let set = handlers.get(type);
      if (!set) handlers.set(type, (set = new Set()));
      set.add(handler as (e: never) => void);
      return () => set.delete(handler as (e: never) => void);
    },
    /** Called after the socket re-opens following a drop; events in between were missed. */
    onReconnected(handler: () => void): () => void {
      reconnectHandlers.add(handler);
      return () => reconnectHandlers.delete(handler);
    },
    /** Point the client at a pasted daemon link (`http://127.0.0.1:7800/?t=…`). */
    connectTo(link: string): boolean {
      const next = parseDaemonLink(link);
      if (!next) return false;
      config.value = next;
      deps.persist?.(next);
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

/** App-wide daemon connection (created on first use, from the browser environment). */
export function useDaemon(): Daemon {
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
