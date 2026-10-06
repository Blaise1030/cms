import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { ApiError } from "@/core/api";
import { useDaemon } from "@/core/daemon";
import type { PermissionDecision, Provider, ToolState, Turn } from "@/core/types";
import { errorMessage } from "@/lib/errors";
import { readPref, writePref } from "@/lib/storage";
import type { PermissionView, ProjectChat, ToolView, TurnView } from "./types";

const sessionKey = (id: string) => `workbench.session.${id}`;
const providerKey = (id: string) => `workbench.provider.${id}`;

export function normalizeToolState(state: string): ToolState {
  const s = state.toLowerCase();
  if (s === "done" || s === "completed" || s === "complete") return "done";
  if (s === "failed" || s === "error") return "failed";
  return "running";
}

export function turnFromHistory(t: Turn): TurnView {
  return {
    id: t.id,
    pending: false,
    sessionId: t.sessionId,
    ts: t.ts,
    userText: t.userText,
    assistantText: t.assistantText,
    tools: t.tools.map((tool, i) => ({
      id: `${t.id}:${i}`,
      tool: tool.tool,
      summary: tool.summary,
      state: normalizeToolState(tool.state),
    })),
    status: t.error ? "error" : "done",
    stopReason: t.stopReason,
    error: t.error,
  };
}

/** Daemon errors meaning "there is no live agent session for this project". */
function isDeadSession(e: unknown): boolean {
  return e instanceof ApiError && (e.code === "ENOENT" || e.code === "AGENT_ERR");
}

function emptyChat(projectId: string): ProjectChat {
  const sessionId = readPref(sessionKey(projectId));
  return {
    sessionId,
    provider: readPref(providerKey(projectId)),
    turns: [],
    permissions: [],
    sessionError: null,
    error: null,
    historyLoaded: false,
    loadingHistory: false,
    sending: false,
    changeRequiredPrompt: null,
  };
}

export const useChatStore = defineStore("chat", () => {
  const daemon = useDaemon();

  const providers = ref<Provider[]>([]);
  const byProject = ref<Record<string, ProjectChat>>({});
  let localSeq = 0;

  function chat(projectId: string): ProjectChat {
    return (byProject.value[projectId] ??= emptyChat(projectId));
  }

  function isBusy(projectId: string): boolean {
    const pc = byProject.value[projectId];
    return !!pc && (pc.sending || pc.turns.some((t) => t.status === "streaming"));
  }

  function findBySession(sessionId: string): { projectId: string; pc: ProjectChat } | null {
    for (const [projectId, pc] of Object.entries(byProject.value)) {
      if (pc.sessionId === sessionId) return { projectId, pc };
    }
    return null;
  }

  function setSession(projectId: string, pc: ProjectChat, sessionId: string | null) {
    pc.sessionId = sessionId;
    writePref(sessionKey(projectId), sessionId);
  }

  async function loadProviders() {
    if (providers.value.length) return;
    try {
      providers.value = await daemon.api.listAgents();
    } catch {
      // The composer surfaces "no providers" itself; nothing else to do here.
    }
  }

  /** Reload turns from the daemon, keeping locally streaming turns the daemon has not settled. */
  async function loadHistory(projectId: string) {
    const pc = chat(projectId);
    pc.loadingHistory = true;
    try {
      const history = (await daemon.api.history(projectId)).map(turnFromHistory);
      const ids = new Set(history.map((t) => t.id));
      const live = pc.turns.filter((t) => t.status === "streaming" && !ids.has(t.id));
      pc.turns = [...history, ...live];
      pc.historyLoaded = true;
    } catch (e) {
      pc.error = errorMessage(e);
    } finally {
      pc.loadingHistory = false;
    }
  }

  async function openProject(projectId: string) {
    const pc = chat(projectId);
    await Promise.all([loadProviders(), pc.historyLoaded ? Promise.resolve() : loadHistory(projectId)]);
  }

  async function startSession(projectId: string): Promise<string> {
    const pc = chat(projectId);
    const provider = pc.provider ?? providers.value[0]?.id;
    if (!provider) throw new Error("No agent providers are configured on the daemon");
    const sessionId = await daemon.api.startSession(projectId, provider);
    pc.provider = provider;
    pc.sessionError = null;
    setSession(projectId, pc, sessionId);
    return sessionId;
  }

  async function ensureSession(projectId: string): Promise<string> {
    const pc = chat(projectId);
    return pc.sessionId ?? (await startSession(projectId));
  }

  async function send(projectId: string, text: string) {
    const body = text.trim();
    const pc = chat(projectId);
    if (!body || isBusy(projectId)) return;

    pc.sending = true;
    pc.error = null;
    pc.turns.push({
      id: `local-${++localSeq}`,
      pending: true,
      sessionId: pc.sessionId,
      ts: Math.floor(Date.now() / 1000),
      userText: body,
      assistantText: "",
      tools: [],
      status: "streaming",
    });
    const turn = pc.turns[pc.turns.length - 1]!;

    try {
      if (providers.value.length === 0) await loadProviders();
      const hadSession = pc.sessionId !== null;
      let sessionId = await ensureSession(projectId);
      let turnId: string;
      try {
        turnId = await daemon.api.prompt(projectId, sessionId, body);
      } catch (e) {
        // A session we did not just create may be gone: the agent died while we were offline
        // (its `chat.error` was missed) or the daemon restarted. Start over once.
        if (!hadSession || !isDeadSession(e)) throw e;
        sessionId = await startSession(projectId);
        turnId = await daemon.api.prompt(projectId, sessionId, body);
      }
      turn.sessionId = sessionId;
      // Events may already have adopted the placeholder id; only claim it if not.
      if (turn.pending) {
        turn.id = turnId;
        turn.pending = false;
      }
    } catch (e) {
      if (e instanceof ApiError && e.code === "CHANGE_REQUIRED") {
        const idx = pc.turns.indexOf(turn);
        if (idx >= 0) pc.turns.splice(idx, 1);
        pc.changeRequiredPrompt = body;
        return;
      }
      turn.status = "error";
      turn.pending = false;
      turn.error = errorMessage(e);
      if (isDeadSession(e)) {
        // Offer recovery (banner + "Restart session") instead of failing every later prompt.
        pc.sessionError = errorMessage(e);
        setSession(projectId, pc, null);
      }
    } finally {
      pc.sending = false;
    }
  }

  /** Consume a pending "Start a change first" prompt, if any. */
  function takeChangeRequired(projectId: string): string | null {
    const pc = byProject.value[projectId];
    if (!pc?.changeRequiredPrompt) return null;
    const text = pc.changeRequiredPrompt;
    pc.changeRequiredPrompt = null;
    return text;
  }

  /** Cancel the streaming turn; the daemon then emits `chat.done` with a cancelled reason. */
  async function stop(projectId: string) {
    const pc = chat(projectId);
    const turn = pc.turns.find((t) => t.status === "streaming" && !t.pending);
    if (!turn) return;
    try {
      await daemon.api.cancelTurn(turn.id);
    } catch (e) {
      pc.error = errorMessage(e);
    }
  }

  async function resolvePermission(
    projectId: string,
    requestId: string,
    decision: PermissionDecision,
  ) {
    const pc = chat(projectId);
    const perm = pc.permissions.find((p) => p.requestId === requestId);
    if (!perm || perm.resolving) return;
    perm.resolving = true;
    try {
      await daemon.api.resolvePermission(requestId, decision);
      pc.permissions = pc.permissions.filter((p) => p.requestId !== requestId);
    } catch (e) {
      perm.resolving = false;
      pc.error = errorMessage(e);
    }
  }

  /** Drop a dead session and start a fresh one (same provider). */
  async function restartSession(projectId: string) {
    const pc = chat(projectId);
    pc.error = null;
    setSession(projectId, pc, null);
    try {
      await startSession(projectId);
    } catch (e) {
      pc.sessionError = errorMessage(e);
    }
  }

  function setProvider(projectId: string, provider: string) {
    const pc = chat(projectId);
    if (pc.provider === provider) return;
    pc.provider = provider;
    writePref(providerKey(projectId), provider);
    // Sessions are per-provider: the next prompt starts a fresh one.
    setSession(projectId, pc, null);
  }

  /** Forget everything about a deleted project. */
  function forget(projectId: string) {
    delete byProject.value[projectId];
    writePref(sessionKey(projectId), null);
    writePref(providerKey(projectId), null);
  }

  // --- daemon events ---------------------------------------------------------------------

  /** Find the turn an event refers to, adopting a local placeholder or creating one. */
  function turnFor(pc: ProjectChat, turnId: string, sessionId: string): TurnView {
    const existing = pc.turns.find((t) => t.id === turnId);
    if (existing) return existing;
    const placeholder = pc.turns.find((t) => t.pending && t.status === "streaming");
    if (placeholder) {
      placeholder.id = turnId;
      placeholder.pending = false;
      return placeholder;
    }
    pc.turns.push({
      id: turnId,
      pending: false,
      sessionId,
      ts: Math.floor(Date.now() / 1000),
      userText: "",
      assistantText: "",
      tools: [],
      status: "streaming",
    });
    return pc.turns[pc.turns.length - 1]!;
  }

  function dropPermissions(pc: ProjectChat, turnId: string) {
    pc.permissions = pc.permissions.filter((p) => p.turnId !== turnId);
  }

  /**
   * Events in a gap (socket drop or `events.lagged`) were dropped: re-sync the history of every
   * project we have shown. Session death cannot be observed directly, so a dead session is
   * recovered on the next prompt (see `send`).
   */
  function resync() {
    for (const [projectId, pc] of Object.entries(byProject.value)) {
      if (pc.historyLoaded) void loadHistory(projectId);
    }
  }

  const offs = [
    daemon.on("chat.chunk", (e) => {
      const hit = findBySession(e.sessionId);
      if (hit) turnFor(hit.pc, e.turnId, e.sessionId).assistantText += e.delta;
    }),
    daemon.on("chat.tool", (e) => {
      const hit = findBySession(e.sessionId);
      if (!hit) return;
      const turn = turnFor(hit.pc, e.turnId, e.sessionId);
      const state = e.state;
      const tool: ToolView | undefined = e.toolCallId
        ? turn.tools.find((t) => t.id === e.toolCallId)
        : [...turn.tools].reverse().find((t) => t.tool === e.tool && t.state === "running");
      if (tool) {
        tool.summary = e.summary || tool.summary;
        tool.state = state;
      } else {
        turn.tools.push({
          id: e.toolCallId ?? `${e.turnId}:${turn.tools.length}`,
          tool: e.tool,
          summary: e.summary,
          state,
        });
      }
    }),
    daemon.on("chat.done", (e) => {
      const hit = findBySession(e.sessionId);
      if (!hit) return;
      const turn = turnFor(hit.pc, e.turnId, e.sessionId);
      turn.status = "done";
      turn.stopReason = e.stopReason;
      dropPermissions(hit.pc, e.turnId);
    }),
    daemon.on("chat.error", (e) => {
      const hit = findBySession(e.sessionId);
      if (!hit) return;
      const { pc, projectId } = hit;
      if (e.turnId === null) {
        // The agent process died with no active turn: the whole session is gone.
        pc.sessionError = e.message;
        pc.permissions = [];
        for (const t of pc.turns) {
          if (t.status === "streaming") {
            t.status = "error";
            t.error = e.message;
          }
        }
        setSession(projectId, pc, null);
        return;
      }
      const turn = turnFor(pc, e.turnId, e.sessionId);
      turn.status = "error";
      turn.error = e.message;
      dropPermissions(pc, e.turnId);
    }),
    daemon.on("permission.request", (e) => {
      const hit = findBySession(e.sessionId);
      if (!hit || hit.pc.permissions.some((p) => p.requestId === e.requestId)) return;
      const perm: PermissionView = {
        requestId: e.requestId,
        sessionId: e.sessionId,
        turnId: e.turnId ?? null,
        tool: e.tool,
        summary: e.summary,
        command: e.command,
        resolving: false,
      };
      hit.pc.permissions.push(perm);
    }),
    daemon.onReconnected(resync),
    daemon.on("events.lagged", resync),
  ];
  onScopeDispose(() => offs.forEach((off) => off()));

  return {
    providers,
    byProject,
    chat,
    isBusy,
    loadProviders,
    loadHistory,
    openProject,
    send,
    takeChangeRequired,
    stop,
    resolvePermission,
    restartSession,
    setProvider,
    forget,
  };
});
