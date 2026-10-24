import { describe, expect, it, vi } from "vitest";
import { createDaemon } from "./daemon";
import type { Api } from "./api";
import type { SocketLike } from "./socket";

function setup(health: () => Promise<unknown>) {
  const sockets: (SocketLike & { url: string })[] = [];
  const timers: (() => void)[] = [];
  const persisted: unknown[] = [];
  const daemon = createDaemon({
    config: { origin: "http://127.0.0.1:7800", token: "tok" },
    api: { health } as unknown as Api,
    persist: (c) => persisted.push(c),
    socketOptions: {
      createSocket: (url) => {
        const s = {
          url,
          onopen: null,
          onmessage: null,
          onclose: null,
          onerror: null,
          close: vi.fn(),
        } as SocketLike & { url: string };
        sockets.push(s);
        return s;
      },
      setTimer: (fn) => timers.push(fn),
      clearTimer: () => {},
      unreachableAfter: 2,
    },
  });
  return { daemon, sockets, timers, persisted };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

describe("createDaemon", () => {
  it("dispatches events by type to subscribers and supports unsubscribe", () => {
    const { daemon, sockets } = setup(async () => ({}));
    const seen: string[] = [];
    const off = daemon.on("chat.turn", (e) => seen.push(`turn:${e.chatId}`));
    daemon.on("fs.change", (e) => seen.push(`fs:${e.path}`));
    daemon.start();
    expect(sockets[0]!.url).toBe("ws://127.0.0.1:7800/ws/events?t=tok");
    sockets[0]!.onopen?.({});
    expect(daemon.status.value).toBe("open");
    const send = (o: unknown) => sockets[0]!.onmessage?.({ data: JSON.stringify(o) });
    send({ type: "chat.turn", projectId: "p", chatId: "c1" });
    send({ type: "fs.change", projectId: "p", path: "x.md", kind: "modify" });
    off();
    send({ type: "chat.turn", projectId: "p", chatId: "c2" });
    expect(seen).toEqual(["turn:c1", "fs:x.md"]);
  });

  it("reports reason=network when nothing answers", async () => {
    const { daemon, sockets, timers } = setup(async () => {
      throw new Error("down");
    });
    daemon.start();
    sockets[0]!.onclose?.({});
    timers.pop()!();
    sockets[1]!.onclose?.({});
    expect(daemon.status.value).toBe("unreachable");
    await flush();
    expect(daemon.reason.value).toBe("network");
  });

  it("reports reason=rejected when health works but the socket does not, and clears on retry", async () => {
    const { daemon, sockets, timers } = setup(async () => ({ status: "ok" }));
    daemon.start();
    sockets[0]!.onclose?.({});
    timers.pop()!();
    sockets[1]!.onclose?.({});
    await flush();
    expect(daemon.reason.value).toBe("rejected");
    daemon.retry();
    expect(daemon.status.value).toBe("connecting");
    expect(daemon.reason.value).toBeNull();
  });

  it("connectTo swaps config, persists it and reconnects to the new origin", () => {
    const { daemon, sockets, persisted } = setup(async () => ({}));
    daemon.start();
    expect(daemon.connectTo("nonsense")).toBe(false);
    expect(daemon.connectTo("https://d.example/?t=new")).toBe(true);
    expect(persisted).toEqual([{ origin: "https://d.example", token: "new" }]);
    expect(sockets.at(-1)!.url).toBe("wss://d.example/ws/events?t=new");
  });

  it("notifies reconnect handlers only after a drop", () => {
    const { daemon, sockets, timers } = setup(async () => ({}));
    const spy = vi.fn();
    daemon.onReconnected(spy);
    daemon.start();
    sockets[0]!.onopen?.({});
    expect(spy).not.toHaveBeenCalled();
    sockets[0]!.onclose?.({});
    timers.pop()!();
    sockets[1]!.onopen?.({});
    expect(spy).toHaveBeenCalledTimes(1);
  });
});
