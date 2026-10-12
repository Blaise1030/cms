// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { ApiError } from "@/core/api";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

/* eslint-disable @typescript-eslint/no-explicit-any */
const h = vi.hoisted(() => ({
  daemon: null as unknown as FakeDaemon,
  term: null as any,
  conn: null as any,
  opts: null as any,
  connects: 0,
}));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("@xterm/xterm/css/xterm.css", () => ({}));
vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    cols = 80;
    rows = 24;
    dataCb: ((d: string) => void) | null = null;
    writes: unknown[] = [];
    writeCbs: (() => void)[] = [];
    focus = vi.fn();
    reset = vi.fn();
    constructor() {
      h.term = this;
    }
    loadAddon() {}
    open() {}
    dispose() {}
    onData(cb: (d: string) => void) {
      this.dataCb = cb;
      return { dispose() {} };
    }
    write(d: unknown, cb?: () => void) {
      this.writes.push(d);
      if (cb) this.writeCbs.push(cb);
    }
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));
vi.mock("./socket", () => ({
  buildTerminalUrl: () => "ws://x",
  connectTerminal: (opts: unknown) => {
    h.connects += 1;
    const connection = {
      send: vi.fn(),
      resize: vi.fn(),
      closed: false,
      close: vi.fn(() => {
        connection.closed = true;
      }),
    };
    h.conn = connection;
    h.opts = {
      ...(opts as Record<string, unknown>),
      onData: (bytes: Uint8Array) => {
        if (!connection.closed) (opts as any).onData(bytes);
      },
    };
    return h.conn;
  },
}));

import TerminalPanel from "./TerminalPanel.vue";
import { usePreviewStore } from "@/modules/preview/store";

let api: Record<string, ReturnType<typeof vi.fn>>;

async function mountPanel(status: { running: boolean; provider?: string } = { running: false }) {
  api = {
    terminalProviders: vi.fn(async () => [
      { id: "cursor", command: ["cursor-agent"] },
      { id: "claude", command: ["claude"] },
      { id: "codex", command: ["codex"] },
    ]),
    terminalStatus: vi.fn(async () => status),
    startTerminal: vi.fn(async () => ({ running: true })),
    stopTerminal: vi.fn(async () => ({ stopped: true })),
  };
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  const w = mount(TerminalPanel, {
    props: { projectId: "p", autoApprove: false },
    attachTo: document.body,
  });
  await flushPromises();
  return w;
}

async function startRunning() {
  const w = await mountPanel();
  await w.find("[data-testid='terminal-start']").trigger("click");
  await flushPromises();
  await flushPromises();
  return w;
}

beforeEach(() => {
  localStorage.clear();
  h.term = h.conn = h.opts = null;
  h.connects = 0;
});

afterEach(() => {
  vi.useRealTimers();
});

describe("TerminalPanel", () => {
  it("shows provider picker and starts the chosen provider", async () => {
    const w = await mountPanel();
    expect(w.find("[data-testid='terminal-panel']").exists()).toBe(true);
    expect(w.find("[data-testid='terminal-provider']").findAll("option").map((o) => o.text())).toEqual([
      "Cursor CLI",
      "Claude",
      "Codex",
    ]);
    await w.find("[data-testid='terminal-provider']").setValue("codex");
    await w.find("[data-testid='terminal-start']").trigger("click");
    await flushPromises();
    expect(api.startTerminal).toHaveBeenCalledWith("p", "codex");
    expect(localStorage.getItem("workbench.terminal.provider")).toBe("codex");
    expect(h.opts).not.toBeNull();
  });

  it("shows the change-required error from start", async () => {
    const w = await mountPanel();
    api.startTerminal.mockRejectedValueOnce(
      new ApiError("CHANGE_REQUIRED", "Start a change first", 409),
    );
    await w.find("[data-testid='terminal-start']").trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='terminal-error']").text()).toContain("Start a change first");
  });

  it("ignores terminal input while replay is being written", async () => {
    await startRunning();
    h.opts.onReplay(new Uint8Array([65]));
    h.term.dataCb("\x1b[?1;2c");
    expect(h.conn.send).not.toHaveBeenCalled();
    h.term.writeCbs.forEach((cb: () => void) => cb());
    h.term.dataCb("x");
    expect(h.conn.send).toHaveBeenCalledWith("x");
  });

  it("pastes a picked element into the terminal", async () => {
    await startRunning();
    usePreviewStore().pick({ tag: "<h1>", selector: "h1", text: "Hi" } as never);
    await flushPromises();
    const sent = h.conn.send.mock.calls.map((c: unknown[]) => c[0]).join("");
    expect(sent).toContain('\x1b[200~[element <h1> "Hi" at `h1`]\x1b[201~');
    expect(h.term.focus).toHaveBeenCalled();
    expect(usePreviewStore().picked).toBeNull();
  });

  it("shows not-found overlay on exit code 127", async () => {
    const w = await startRunning();
    h.opts.onExit(127);
    await flushPromises();
    expect(w.find("[data-testid='terminal-ended']").text()).toContain("`cursor-agent` not found on PATH");
  });

  it("reconnects when the socket closes without an exit", async () => {
    await startRunning();
    vi.useFakeTimers();
    expect(h.connects).toBe(1);
    h.opts.onClose();
    await vi.advanceTimersByTimeAsync(500);
    expect(h.connects).toBe(2);
  });

  it("stops reconnecting and shows Restart when terminal.exited arrives", async () => {
    const w = await startRunning();
    vi.useFakeTimers();
    h.opts.onClose();
    h.daemon.emit({ type: "terminal.exited", projectId: "p", code: 1 });
    await flushPromises();
    expect(w.find("[data-testid='terminal-ended']").text()).toContain("Session ended (code 1)");
    expect(w.find("[data-testid='terminal-restart']").exists()).toBe(true);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(h.connects).toBe(1);
  });

  it("drains terminal output after the daemon exit event", async () => {
    const w = await startRunning();
    h.daemon.emit({ type: "terminal.exited", projectId: "p", code: 1 });
    h.opts.onData(new TextEncoder().encode("FINAL-ERROR"));
    await new Promise((resolve) => requestAnimationFrame(resolve));
    expect(
      h.term.writes.some(
        (data: unknown) =>
          data instanceof Uint8Array && new TextDecoder().decode(data).includes("FINAL-ERROR"),
      ),
    ).toBe(true);
    expect(w.find("[data-testid='terminal-ended']").text()).toContain("Session ended (code 1)");
  });

  it("ignores terminal.exited for another project and still reconnects", async () => {
    const w = await startRunning();
    vi.useFakeTimers();
    h.opts.onClose();
    h.daemon.emit({ type: "terminal.exited", projectId: "other", code: 9 });
    await flushPromises();
    await vi.advanceTimersByTimeAsync(500);
    expect(w.find("[data-testid='terminal-ended']").exists()).toBe(false);
    expect(h.connects).toBe(2);
  });
});
