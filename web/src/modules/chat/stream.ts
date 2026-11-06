import type { DaemonConfig } from "@/core/config";
import type { UiChunk } from "@/core/types";

/** `GET /api/chat/{project}/{chat}/stream` — the same token convention as `/ws/events`. */
export function buildChatStreamUrl(
  config: DaemonConfig,
  projectId: string,
  chatId: string,
): string {
  const url = new URL(
    `/api/chat/${encodeURIComponent(projectId)}/${encodeURIComponent(chatId)}/stream`,
    config.origin,
  );
  if (config.token) url.searchParams.set("t", config.token);
  return url.toString();
}

/** The subset of `EventSource` we use (so tests can inject a fake). */
export interface ChatStreamSource {
  onopen: ((ev: unknown) => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  onerror: ((ev: unknown) => void) | null;
  close(): void;
}
export type ChatStreamFactory = (url: string) => ChatStreamSource;

export interface ChatStreamOptions {
  /** Evaluated on connect, so origin/token changes are picked up. */
  url: () => string;
  /**
   * Chunks already rendered before the first connection. The daemon replays the whole
   * transcript on every connection, so these leading frames are duplicates.
   */
  skipOnFirstOpen?: () => number;
  /** Fired before a reconnected stream replays from the start: rebuild local state here. */
  onReset?: () => void;
  onChunk?: (chunk: UiChunk) => void;
  onError?: () => void;
  createSource?: ChatStreamFactory;
}

export interface ChatStream {
  /** Attach the source. `false` when the browser has no `EventSource` (or one could not be made). */
  connect(): boolean;
  close(): void;
}

/** One SSE frame's JSON payload, or `null` for anything that is not a chunk. */
export function decodeChunk(raw: unknown): UiChunk | null {
  if (typeof raw !== "string") return null;
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return null;
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const type = (value as Record<string, unknown>).type;
  if (typeof type !== "string") return null;
  return value as UiChunk;
}

export function createChatStream(opts: ChatStreamOptions): ChatStream {
  const createSource: ChatStreamFactory =
    opts.createSource ?? ((url) => new EventSource(url) as unknown as ChatStreamSource);
  let source: ChatStreamSource | null = null;
  let opened = false;
  let skip = 0;

  function detach(s: ChatStreamSource) {
    s.onopen = s.onmessage = s.onerror = null;
  }

  return {
    connect() {
      if (source) return true;
      let s: ChatStreamSource;
      try {
        s = createSource(opts.url());
      } catch {
        return false;
      }
      source = s;
      s.onopen = () => {
        if (source !== s) return;
        if (opened) {
          skip = 0;
          opts.onReset?.();
        } else {
          opened = true;
          skip = opts.skipOnFirstOpen?.() ?? 0;
        }
      };
      s.onmessage = (ev) => {
        if (source !== s) return;
        const chunk = decodeChunk(ev.data);
        if (!chunk) return;
        if (skip > 0) {
          skip -= 1;
          return;
        }
        opts.onChunk?.(chunk);
      };
      s.onerror = () => {
        if (source !== s) return;
        opts.onError?.();
      };
      return true;
    },
    close() {
      if (!source) return;
      detach(source);
      source.close();
      source = null;
    },
  };
}
