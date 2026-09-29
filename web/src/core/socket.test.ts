import { describe, expect, it } from "vitest";
import type { ConnectionStatus, DaemonEvent } from "./types";
import {
  buildEventsUrl,
  createEventSocket,
  decodeEvent,
  type EventSocketOptions,
  type SocketLike,
} from "./socket";

describe("buildEventsUrl", () => {
  it("swaps the scheme and appends the token", () => {
    expect(buildEventsUrl({ origin: "http://127.0.0.1:7800", token: "a b" })).toBe(
      "ws://127.0.0.1:7800/ws/events?t=a+b",
    );
    expect(buildEventsUrl({ origin: "https://daemon.example" })).toBe(
      "wss://daemon.example/ws/events",
    );
  });
});

describe("decodeEvent", () => {
  it("decodes tagged events from JSON text", () => {
    const ev = decodeEvent(
      '{"type":"chat.chunk","sessionId":"s","turnId":"t","delta":"hi","projectId":"p"}',
    );
    expect(ev).toEqual({
      type: "chat.chunk",
      sessionId: "s",
      turnId: "t",
      delta: "hi",
      projectId: "p",
    });
  });

  it("decodes every documented event type", () => {
    const frames: Record<string, unknown>[] = [
      { type: "chat.tool", sessionId: "s", turnId: "t", tool: "edit", state: "done", summary: "x" },
      { type: "chat.plan", sessionId: "s", turnId: "t", entries: [] },
      { type: "chat.done", sessionId: "s", turnId: "t", stopReason: "end_turn" },
      { type: "chat.error", sessionId: "s", turnId: "t", message: "boom" },
      { type: "permission.request", sessionId: "s", requestId: "r", tool: "execute", command: "ls" },
      { type: "devserver.status", projectId: "p", state: "ready", port: 4000 },
      { type: "checkpoint.created", projectId: "p", sha: "abc", summary: "s" },
      { type: "fs.change", projectId: "p", path: "a.md", kind: "modify" },
      { type: "events.lagged", missed: 3 },
    ];
    for (const f of frames) expect(decodeEvent(JSON.stringify(f))).toEqual(f);
  });

  it("returns null for junk, unknown types and malformed events", () => {
    expect(decodeEvent("not json")).toBeNull();
    expect(decodeEvent("[]")).toBeNull();
    expect(decodeEvent("null")).toBeNull();
    expect(decodeEvent(new ArrayBuffer(1))).toBeNull();
    expect(decodeEvent('{"type":"future.thing"}')).toBeNull();
    expect(decodeEvent('{"type":"toString"}')).toBeNull();
    expect(decodeEvent('{"type":"chat.chunk","sessionId":"s","turnId":"t"}')).toBeNull();
    expect(decodeEvent('{"type":"events.lagged","missed":"3"}')).toBeNull();
    expect(decodeEvent({ type: "fs.change", projectId: "p", path: "a", kind: "x" })).not.toBeNull();
  });
});

class FakeSocket implements SocketLike {
  onopen: SocketLike["onopen"] = null;
  onmessage: SocketLike["onmessage"] = null;
  onclose: SocketLike["onclose"] = null;
  onerror: SocketLike["onerror"] = null;
  closed = false;
  readonly url: string;
  constructor(url: string) {
    this.url = url;
  }
  close() {
    this.closed = true;
  }
  open() {
    this.onopen?.({});
  }
  message(data: unknown) {
    this.onmessage?.({ data });
  }
  drop() {
    this.onclose?.({});
  }
}

function harness(overrides: Partial<EventSocketOptions> = {}) {
  const sockets: FakeSocket[] = [];
  const timers: { fn: () => void; ms: number; cleared: boolean }[] = [];
  const statuses: ConnectionStatus[] = [];
  const events: DaemonEvent[] = [];
  let reconnected = 0;
  let url = "ws://d/ws/events?t=1";
  const socket = createEventSocket({
    url: () => url,
    onEvent: (e) => events.push(e),
    onStatus: (s) => statuses.push(s),
    onReconnected: () => reconnected++,
    createSocket: (u) => {
      const s = new FakeSocket(u);
      sockets.push(s);
      return s;
    },
    setTimer: (fn, ms) => {
      const t = { fn, ms, cleared: false };
      timers.push(t);
      return t;
    },
    clearTimer: (h) => {
      (h as { cleared: boolean }).cleared = true;
    },
    initialDelayMs: 100,
    maxDelayMs: 400,
    unreachableAfter: 3,
    ...overrides,
  });
  const fire = () => {
    const t = timers.filter((x) => !x.cleared).pop();
    if (!t) throw new Error("no pending timer");
    t.cleared = true;
    t.fn();
  };
  return {
    socket,
    sockets,
    timers,
    statuses,
    events,
    fire,
    setUrl: (u: string) => (url = u),
    reconnected: () => reconnected,
  };
}

describe("createEventSocket", () => {
  it("opens, reports status and delivers decoded events (dropping junk)", () => {
    const h = harness();
    h.socket.connect();
    expect(h.statuses).toEqual([]); // already "connecting"
    h.sockets[0]!.open();
    expect(h.socket.status).toBe("open");
    h.sockets[0]!.message('{"type":"fs.change","projectId":"p","path":"a","kind":"modify"}');
    h.sockets[0]!.message("garbage");
    h.sockets[0]!.message('{"type":"nope"}');
    expect(h.events).toEqual([{ type: "fs.change", projectId: "p", path: "a", kind: "modify" }]);
  });

  it("reconnects with exponential backoff, capped, and signals reconnection", () => {
    const h = harness();
    h.socket.connect();
    h.sockets[0]!.open();

    h.sockets[0]!.drop();
    expect(h.socket.status).toBe("reconnecting");
    expect(h.timers.at(-1)!.ms).toBe(100);
    h.fire();
    expect(h.sockets).toHaveLength(2);

    h.sockets[1]!.drop(); // failure 2
    expect(h.timers.at(-1)!.ms).toBe(200);
    h.fire();
    h.sockets[2]!.drop(); // failure 3 → unreachable
    expect(h.socket.status).toBe("unreachable");
    expect(h.timers.at(-1)!.ms).toBe(400);
    h.fire();
    h.sockets[3]!.drop();
    expect(h.timers.at(-1)!.ms).toBe(400); // capped, keeps retrying

    h.fire();
    expect(h.reconnected()).toBe(0);
    h.sockets[4]!.open();
    expect(h.socket.status).toBe("open");
    expect(h.reconnected()).toBe(1);

    // backoff resets after a successful open
    h.sockets[4]!.drop();
    expect(h.timers.at(-1)!.ms).toBe(100);
  });

  it("goes unreachable when the first connection never opens, without onReconnected", () => {
    const h = harness();
    h.socket.connect();
    for (let i = 0; i < 3; i++) {
      h.sockets[i]!.drop();
      if (i < 2) {
        expect(h.socket.status).toBe("connecting");
        h.fire();
      }
    }
    expect(h.socket.status).toBe("unreachable");
    expect(h.statuses).toEqual(["unreachable"]);
    h.fire();
    h.sockets[3]!.open();
    expect(h.socket.status).toBe("open");
    expect(h.reconnected()).toBe(0);
  });

  it("treats error events like closes, only once per socket", () => {
    const h = harness();
    h.socket.connect();
    h.sockets[0]!.onerror?.({});
    h.sockets[0]!.onclose?.({});
    expect(h.timers.filter((t) => !t.cleared)).toHaveLength(1);
  });

  it("retryNow reconnects immediately, cancels the pending backoff and resets failures", () => {
    const h = harness();
    h.socket.connect();
    h.sockets[0]!.drop();
    expect(h.sockets).toHaveLength(1);
    h.socket.retryNow();
    expect(h.timers.filter((t) => !t.cleared)).toHaveLength(0);
    expect(h.sockets).toHaveLength(2);
    expect(h.socket.status).toBe("connecting");
  });

  it("re-evaluates the url on every attempt", () => {
    const h = harness();
    h.socket.connect();
    h.setUrl("ws://other/ws/events");
    h.sockets[0]!.drop();
    h.fire();
    expect(h.sockets[1]!.url).toBe("ws://other/ws/events");
  });

  it("close() stops retrying and ignores late events from old sockets", () => {
    const h = harness();
    h.socket.connect();
    const first = h.sockets[0]!;
    first.open();
    h.socket.close();
    expect(first.closed).toBe(true);
    first.message('{"type":"events.lagged","missed":1}');
    first.drop();
    expect(h.events).toHaveLength(0);
    expect(h.timers.filter((t) => !t.cleared)).toHaveLength(0);
    expect(h.sockets).toHaveLength(1);
  });

  it("survives a throwing socket constructor by scheduling a retry", () => {
    let n = 0;
    const h = harness({
      createSocket: () => {
        if (n++ === 0) throw new Error("bad url");
        return new FakeSocket("x");
      },
    });
    h.socket.connect();
    expect(h.timers.filter((t) => !t.cleared)).toHaveLength(1);
    h.fire();
    expect(n).toBe(2);
  });
});
