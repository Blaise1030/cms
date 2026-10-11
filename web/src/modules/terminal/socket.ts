import type { DaemonConfig } from "@/core/config";
import type { SocketFactory, SocketLike } from "@/core/socket";

/** `http(s)://host` → `ws(s)://host/api/terminal/{project}/ws[?t=token]`. */
export function buildTerminalUrl(config: DaemonConfig, projectId: string): string {
  const url = new URL(`/api/terminal/${encodeURIComponent(projectId)}/ws`, config.origin);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  if (config.token) url.searchParams.set("t", config.token);
  return url.toString();
}

export interface TerminalSocketOptions {
  url: string;
  /** The first binary frame: scrollback replay. */
  onReplay(bytes: Uint8Array): void;
  onData(bytes: Uint8Array): void;
  onExit(code: number | null): void;
  onClose(): void;
  createSocket?: SocketFactory;
}

export interface TerminalConnection {
  send(data: Uint8Array | string): void;
  resize(cols: number, rows: number): void;
  close(): void;
}

type SendableSocket = SocketLike & { binaryType?: string; send(data: string | Uint8Array): void };

export function connectTerminal(opts: TerminalSocketOptions): TerminalConnection {
  const socket = (
    opts.createSocket ?? ((url: string) => new WebSocket(url) as unknown as SocketLike)
  )(opts.url) as SendableSocket;
  socket.binaryType = "arraybuffer";
  let first = true;
  let closed = false;

  socket.onmessage = (ev) => {
    const data = ev.data;
    if (typeof data === "string") {
      try {
        const msg = JSON.parse(data) as { exit?: boolean; code?: number | null };
        if (msg.exit) opts.onExit(typeof msg.code === "number" ? msg.code : null);
      } catch {
        // ignore malformed control frames
      }
      return;
    }
    const bytes = new Uint8Array(data as ArrayBuffer);
    if (first) {
      first = false;
      opts.onReplay(bytes);
    } else {
      opts.onData(bytes);
    }
  };
  const lost = () => {
    if (closed) return;
    closed = true;
    opts.onClose();
  };
  socket.onclose = lost;
  socket.onerror = lost;

  return {
    send(data) {
      try {
        // Text frames are control messages; keystrokes always go as binary.
        socket.send(typeof data === "string" ? new TextEncoder().encode(data) : data);
      } catch {
        // socket not open; the reconnect path will recover
      }
    },
    resize(cols, rows) {
      try {
        socket.send(JSON.stringify({ cols, rows }));
      } catch {
        // see send()
      }
    },
    close() {
      closed = true;
      socket.onmessage = socket.onclose = socket.onerror = null;
      socket.close();
    },
  };
}
