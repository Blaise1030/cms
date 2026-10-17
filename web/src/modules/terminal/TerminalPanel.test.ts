// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { ApiError } from "@/core/api";
import type { TerminalSession } from "@/core/types";
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
    options: any;
    constructor(options: any) {
      this.options = { ...options };
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
import { useAgentOutboxStore } from "@/modules/chat/outbox";
import { useTheme } from "@/lib/theme";
import { useTerminalStore } from "./store";
import { useChatStore } from "@/modules/chat/store";

enableAutoUnmount(afterEach);

let api: Record<string, ReturnType<typeof vi.fn>>;
let sessions: TerminalSession[] = [];

async function mountPanel(
  status: { running: boolean; provider?: string; sessionId?: string } = { running: false },
  extra: { active?: boolean } = {},
) {
  api = {
    terminalSessions: vi.fn(async () => sessions),
    terminalProviders: vi.fn(async () => [
      { id: "cursor", command: ["cursor-agent"] },
      { id: "claude", command: ["claude"] },
      { id: "codex", command: ["codex"] },
    ]),
    terminalStatus: vi.fn(async () => status),
    startTerminal: vi.fn(async () => ({ running: true })),
    stopTerminal: vi.fn(async () => ({ stopped: true })),
    attachTerminalImage: vi.fn(),
  };
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  const w = mount(TerminalPanel, {
    props: { projectId: "p", ...extra },
    attachTo: document.body,
  });
  await flushPromises();
  return w;
}

/** Reka parks popper content off-screen until it is positioned against its trigger. */
function isAnchored(el: Element): boolean {
  const wrapper = el.closest<HTMLElement>("[data-reka-popper-content-wrapper]")!;
  return !wrapper.style.transform.includes("-200%");
}

async function pressStart(w: VueWrapper) {
  await w.find("[data-testid='terminal-start']").trigger("click");
  await flushPromises();
  await flushPromises();
}

async function startRunning() {
  const w = await mountPanel();
  await pressStart(w);
  return w;
}

/** The panel's root, wherever it is rendered (it moves to `<body>` while floating). */
function panel(): HTMLElement {
  return document.body.querySelector<HTMLElement>("[data-testid='terminal-panel']")!;
}

function pointer(target: EventTarget, type: string, x: number, y: number) {
  target.dispatchEvent(new PointerEvent(type, { clientX: x, clientY: y, pointerType: "mouse", bubbles: true }));
}

/** The socket's first frame arrives and xterm finishes writing it. */
function replayDone() {
  h.opts.onReplay(new Uint8Array());
  h.term.writeCbs.splice(0).forEach((cb: () => void) => cb());
}

function sent(): string {
  return h.conn.send.mock.calls.map((c: unknown[]) => c[0]).join("");
}

beforeEach(() => {
  localStorage.clear();
  h.term = h.conn = h.opts = null;
  h.connects = 0;
  sessions = [];
});

/** The chat tab's saved active conversation, as a previous visit left it. */
function activeChat(id: string) {
  localStorage.setItem("workbench.chat.active.p", id);
}

function session(id: string, provider: string, extra: Partial<TerminalSession> = {}): TerminalSession {
  return { id, provider, createdAt: 1, updatedAt: 1, resumable: true, ...extra };
}

describe("TerminalPanel sessions", () => {
  it("lists sessions in a popover and resumes a past one", async () => {
    sessions = [
      session("s1", "claude", { title: "Add pricing" }),
      session("s2", "codex", { title: "Fix footer" }),
      session("s3", "fake", { resumable: false }),
    ];
    const w = await mountPanel({ running: true, provider: "claude", sessionId: "s1" });
    expect(w.find("[data-testid='terminal-title']").text()).toBe("Add pricing");
    await w.find("[data-testid='terminal-sessions-toggle']").trigger("click");
    await flushPromises();
    const list = document.body.querySelector("[data-testid='terminal-sessions']")!;
    expect(list.closest("[data-slot='popover-content']")).not.toBeNull();
    expect(isAnchored(list)).toBe(true);
    expect(w.find("[data-testid='terminal-sessions']").exists()).toBe(false);
    const items = [...list.querySelectorAll<HTMLButtonElement>("[data-testid='terminal-session']")];
    expect(items.map((i) => i.querySelector("span")!.textContent!.trim())).toEqual([
      "Add pricing",
      "Fix footer",
      "fake session",
    ]);
    expect(items[0]!.textContent).toContain("Running");
    expect(items[2]!.disabled).toBe(true);

    api.startTerminal.mockResolvedValueOnce({ running: true, sessionId: "s2" });
    items[1]!.click();
    await flushPromises();
    await flushPromises();
    expect(api.stopTerminal).toHaveBeenCalledWith("p");
    expect(api.startTerminal).toHaveBeenCalledWith("p", "codex", "s2");
    expect(document.body.querySelector("[data-testid='terminal-sessions']")).toBeNull();
    expect(w.find("[data-testid='terminal-title']").text()).toBe("Fix footer");

    // The stopped session's exit arrives late; the resumed one keeps running.
    h.daemon.emit({ type: "terminal.exited", projectId: "p", code: 0, sessionId: "s1" });
    await flushPromises();
    expect(w.find("[data-testid='terminal-ended']").exists()).toBe(false);
  });

  it("stops the live session and shows the picker for a new one", async () => {
    sessions = [session("s1", "claude")];
    const w = await mountPanel({ running: true, provider: "claude", sessionId: "s1" });
    await w.find("[data-testid='terminal-new-session']").trigger("click");
    await flushPromises();
    expect(api.stopTerminal).toHaveBeenCalledWith("p");
    expect(w.find("[data-testid='terminal-start']").exists()).toBe(true);
    expect(w.find("[data-testid='terminal-title']").text()).toBe("New session");
  });

  it("offers to continue the chat tab's conversation in its own provider", async () => {
    sessions = [session("s1", "codex"), session("c1", "claude", { title: "Add pricing" })];
    activeChat("c1");
    const w = await mountPanel();
    const cont = w.find("[data-testid='terminal-continue']");
    expect(cont.text()).toBe("Continue Add pricing (Claude)");
    expect(w.find("[data-testid='terminal-provider']").exists()).toBe(false);
    expect(w.find("[data-testid='terminal-start']").exists()).toBe(false);
    expect(w.find("[data-testid='terminal-title']").text()).toBe("Add pricing");
    await cont.trigger("click");
    await flushPromises();
    await flushPromises();
    expect(api.startTerminal).toHaveBeenCalledWith("p", "claude", "c1");
  });

  it("names an untitled conversation generically", async () => {
    sessions = [session("c1", "codex")];
    activeChat("c1");
    const w = await mountPanel();
    expect(w.find("[data-testid='terminal-continue']").text()).toBe("Continue this conversation (Codex)");
  });

  it("New session starts the picked provider fresh instead of continuing", async () => {
    sessions = [session("c1", "claude")];
    activeChat("c1");
    const w = await mountPanel();
    await w.find("[data-testid='terminal-new-session']").trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='terminal-continue']").exists()).toBe(false);
    const choices = w.find("[data-testid='terminal-provider']").findAll("[data-slot='questionnaire-choice']");
    await choices[2]!.find("input").setValue(true);
    await pressStart(w);
    expect(api.startTerminal).toHaveBeenCalledTimes(1);
    expect(api.startTerminal).toHaveBeenCalledWith("p", "codex");
  });

  it("shows the provider picker when the chat has no session to continue", async () => {
    sessions = [session("s1", "claude")];
    const w = await mountPanel();
    expect(w.find("[data-testid='terminal-continue']").exists()).toBe(false);
    expect(w.find("[data-testid='terminal-provider']").exists()).toBe(true);
    await pressStart(w);
    expect(api.startTerminal).toHaveBeenCalledWith("p", "cursor");
  });
});

describe("TerminalPanel inactive", () => {
  it("leaves the outbox alone until it becomes active", async () => {
    const w = await mountPanel({ running: true, provider: "claude", sessionId: "s1" }, { active: false });
    replayDone();
    useAgentOutboxStore().send({ kind: "text", text: "hello" });
    useAgentOutboxStore().send({ kind: "text", text: "go", submit: true });
    await flushPromises();
    expect(sent()).toBe("");
    expect(useAgentOutboxStore().messages).toHaveLength(2);

    await w.setProps({ active: true });
    await flushPromises();
    expect(sent()).toContain("\x1b[200~hello\x1b[201~");
    expect(useAgentOutboxStore().messages).toEqual([]);
  });

  it("does not start a session for a request while inactive", async () => {
    await mountPanel(undefined, { active: false });
    useAgentOutboxStore().send({ kind: "text", text: "go", submit: true });
    await flushPromises();
    expect(api.startTerminal).not.toHaveBeenCalled();
  });
});

describe("TerminalPanel floating", () => {
  it("pops out into a floating window over the page and docks back", async () => {
    const w = await mountPanel();
    const host = w.element.parentElement!;
    await w.find("[data-testid='terminal-float-toggle']").trigger("click");
    expect(useTerminalStore().floating).toBe(true);
    expect(panel().parentElement).toBe(document.body);
    expect(panel().dataset.floating).toBe("true");
    expect(panel().querySelector("[data-testid='terminal-float-toggle']")!.getAttribute("aria-label")).toBe("Dock");

    (panel().querySelector("[data-testid='terminal-float-toggle']") as HTMLElement).click();
    await flushPromises();
    expect(useTerminalStore().floating).toBe(false);
    expect(host.contains(panel())).toBe(true);
    expect(panel().dataset.floating).toBeUndefined();
  });

  it("keeps the running session when popping out", async () => {
    const w = await startRunning();
    await w.find("[data-testid='terminal-float-toggle']").trigger("click");
    await flushPromises();
    expect(h.connects).toBe(1);
    expect(h.conn.close).not.toHaveBeenCalled();
  });

  it("drags by its header", async () => {
    const w = await mountPanel();
    await w.find("[data-testid='terminal-float-toggle']").trigger("click");
    await flushPromises();
    const start = { left: parseFloat(panel().style.left), top: parseFloat(panel().style.top) };
    const header = panel().querySelector("[data-testid='terminal-header']")!;
    pointer(header, "pointerdown", 300, 200);
    pointer(window, "pointermove", 260, 230);
    pointer(window, "pointerup", 260, 230);
    await flushPromises();
    expect(parseFloat(panel().style.left)).toBe(start.left - 40);
    expect(parseFloat(panel().style.top)).toBe(start.top + 30);
  });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("TerminalPanel", () => {
  it("asks which agent to use and starts the chosen one", async () => {
    const w = await mountPanel();
    const choices = w.find("[data-testid='terminal-provider']").findAll("[data-slot='questionnaire-choice']");
    const radios = () => choices.map((c) => (c.find("input[type='radio']").element as HTMLInputElement).checked);
    expect(choices.map((c) => c.text())).toEqual(["Cursor CLI", "Claude", "Codex"]);
    expect(radios()).toEqual([true, false, false]);
    await choices[2]!.find("input").setValue(true);
    await flushPromises();
    expect(radios()).toEqual([false, false, true]);
    await w.find("[data-testid='terminal-start']").trigger("click");
    await flushPromises();
    expect(api.startTerminal).toHaveBeenCalledWith("p", "codex");
  });

  it("disables Start while a chat turn runs", async () => {
    const w = await mountPanel();
    h.daemon.emit({ type: "terminal.busy", projectId: "p", busy: true });
    await flushPromises();
    const start = w.find<HTMLButtonElement>("[data-testid='terminal-start']");
    expect(start.element.disabled).toBe(true);
    expect(start.element.closest("[title]")?.getAttribute("title")).toBe("The agent is busy in Chat");
    await start.trigger("click");
    await flushPromises();
    expect(api.startTerminal).not.toHaveBeenCalled();

    h.daemon.emit({ type: "terminal.busy", projectId: "p", busy: false });
    await flushPromises();
    expect(w.find<HTMLButtonElement>("[data-testid='terminal-start']").element.disabled).toBe(false);
    expect(w.find("[data-testid='terminal-start']").element.closest("[title]")).toBeNull();
  });

  it("disables Continue while the chat store has a running turn, and doesn't start for requests", async () => {
    sessions = [session("c1", "claude")];
    activeChat("c1");
    const w = await mountPanel();
    useChatStore().running.p = "c1";
    await flushPromises();
    const cont = w.find<HTMLButtonElement>("[data-testid='terminal-continue']");
    expect(cont.element.disabled).toBe(true);
    expect(cont.element.closest("[title]")?.getAttribute("title")).toBe("The agent is busy in Chat");
    useAgentOutboxStore().send({ kind: "text", text: "Sync please", submit: true });
    await flushPromises();
    expect(api.startTerminal).not.toHaveBeenCalled();

    useChatStore().running.p = null;
    await flushPromises();
    await flushPromises();
    expect(api.startTerminal).toHaveBeenCalledWith("p", "claude", "c1");
  });

  it("shows the change-required error from start", async () => {
    const w = await mountPanel();
    api.startTerminal.mockRejectedValueOnce(
      new ApiError("CHANGE_REQUIRED", "Start a change first", 409),
    );
    await pressStart(w);
    expect(w.find("[data-testid='terminal-error']").text()).toContain("Start a change first");
  });

  it("follows the app theme, including after the terminal opens", async () => {
    const { setPreference } = useTheme();
    setPreference("light");
    await startRunning();
    expect(h.term.options.theme.yellow).toBeDefined();
    setPreference("dark");
    await flushPromises();
    expect(h.term.options.theme.yellow).toBeUndefined();
    setPreference("system");
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
    replayDone();
    usePreviewStore().pick({ tag: "<h1>", selector: "h1", text: "Hi" } as never);
    await flushPromises();
    expect(sent()).toContain('\x1b[200~[element <h1> "Hi" at `h1`]\x1b[201~');
    expect(h.term.focus).toHaveBeenCalled();
    expect(useAgentOutboxStore().messages).toEqual([]);
  });

  it("uploads dropped images and pastes their paths for the agent", async () => {
    await startRunning();
    replayDone();
    api.attachTerminalImage!
      .mockResolvedValueOnce("/state/p/attachments/a.png")
      .mockResolvedValueOnce("/my state/b.jpg");
    const png = new File(["1"], "a.png", { type: "image/png" });
    const jpg = new File(["2"], "b.jpg", { type: "image/jpeg" });
    const txt = new File(["3"], "c.txt", { type: "text/plain" });
    const drop = new Event("drop", { bubbles: true, cancelable: true });
    Object.defineProperty(drop, "dataTransfer", { value: { files: [png, txt, jpg], types: ["Files"] } });
    panel().dispatchEvent(drop);
    await flushPromises();
    expect(drop.defaultPrevented).toBe(true);
    expect(api.attachTerminalImage.mock.calls).toEqual([["p", png], ["p", jpg]]);
    expect(sent()).toBe("\x1b[200~/state/p/attachments/a.png /my\\ state/b.jpg \x1b[201~");
  });

  it("delivers a pick made before the agent started once its session is ready", async () => {
    const w = await mountPanel();
    usePreviewStore().pick({ tag: "<h1>", selector: "h1", text: "Hi" } as never);
    await pressStart(w);
    expect(sent()).toBe("");
    replayDone();
    await flushPromises();
    expect(sent()).toContain('[element <h1> "Hi" at `h1`]');
  });

  it("holds messages until the replay has been written", async () => {
    await startRunning();
    useAgentOutboxStore().send({ kind: "text", text: "hello" });
    await flushPromises();
    expect(sent()).toBe("");
    replayDone();
    await flushPromises();
    expect(sent()).toBe("\x1b[200~hello\x1b[201~");
  });

  it("starts the agent for a request and submits it once the agent has booted", async () => {
    await mountPanel();
    vi.useFakeTimers();
    useAgentOutboxStore().send({ kind: "text", text: "Sync please", submit: true });
    await vi.advanceTimersByTimeAsync(0);
    expect(api.startTerminal).toHaveBeenCalledWith("p", "cursor");
    replayDone();
    await vi.advanceTimersByTimeAsync(1000);
    expect(sent()).toBe("");
    await vi.advanceTimersByTimeAsync(4000);
    expect(sent()).toBe("\x1b[200~Sync please\x1b[201~\r");
    expect(useAgentOutboxStore().messages).toEqual([]);
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
