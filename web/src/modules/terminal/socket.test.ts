import { describe, expect, it, vi } from "vitest";
import { buildTerminalUrl, connectTerminal } from "./socket";

class FakeSocket {
  binaryType = "";
  sent: unknown[] = [];
  onopen: ((e: unknown) => void) | null = null;
  onmessage: ((e: { data: unknown }) => void) | null = null;
  onclose: ((e: unknown) => void) | null = null;
  onerror: ((e: unknown) => void) | null = null;
  closed = false;
  send(d: unknown) {
    this.sent.push(d);
  }
  close() {
    this.closed = true;
  }
}

function setup() {
  const sock = new FakeSocket();
  const cb = { onReplay: vi.fn(), onData: vi.fn(), onExit: vi.fn(), onClose: vi.fn() };
  const conn = connectTerminal({ url: "ws://x", ...cb, createSocket: () => sock as never });
  return { sock, cb, conn };
}
const buf = (...b: number[]) => new Uint8Array(b).buffer;

describe("connectTerminal", () => {
  it("routes the first binary frame to onReplay and later ones to onData", () => {
    const { sock, cb } = setup();
    expect(sock.binaryType).toBe("arraybuffer");
    sock.onmessage!({ data: buf(1) });
    sock.onmessage!({ data: buf(2) });
    expect(cb.onReplay).toHaveBeenCalledTimes(1);
    expect(cb.onReplay.mock.calls[0][0]).toEqual(new Uint8Array([1]));
    expect(cb.onData.mock.calls[0][0]).toEqual(new Uint8Array([2]));
  });

  it("writes binary frames as Uint8Array", () => {
    const { sock, cb } = setup();
    sock.onmessage!({ data: buf(0) });
    // "é" = c3 a9 split across two frames
    sock.onmessage!({ data: buf(0xc3) });
    sock.onmessage!({ data: buf(0xa9) });
    const [a, b] = cb.onData.mock.calls.map((c) => c[0]);
    expect(a).toBeInstanceOf(Uint8Array);
    expect([...a, ...b]).toEqual([0xc3, 0xa9]);
  });

  it("sends resize as JSON text", () => {
    const { sock, conn } = setup();
    conn.resize(100, 30);
    expect(sock.sent).toEqual(['{"cols":100,"rows":30}']);
  });

  it("calls onExit for an exit control frame", () => {
    const { sock, cb } = setup();
    sock.onmessage!({ data: '{"exit":true,"code":127}' });
    expect(cb.onExit).toHaveBeenCalledWith(127);
  });

  it("buildTerminalUrl uses ws scheme, path and token", () => {
    expect(buildTerminalUrl({ origin: "http://127.0.0.1:7800", token: "tok" }, "my proj")).toBe(
      "ws://127.0.0.1:7800/api/terminal/my%20proj/ws?t=tok",
    );
  });
});
