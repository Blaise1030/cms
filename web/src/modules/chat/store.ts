import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { ChatSummary } from "@/core/types";
import { readPref, writePref } from "@orso/utils";

const activeKey = (projectId: string) => `orso.chat.active.${projectId}`;

export interface QueuedPrompt { id: string; body: string; preview: string }

export const useChatStore = defineStore("chat", () => {
  const daemon = useDaemon();

  /** Each loaded project's chats, as the daemon lists them. */
  const chats = ref<Record<string, ChatSummary[]>>({});
  /** The chat with a turn in flight, per loaded project. */
  const running = ref<Record<string, string | null>>({});
  /** Every running chat, including concurrent sessions in the same project. */
  const runningChats = ref<Record<string, Record<string, boolean>>>({});
  /** Active chat ids chosen this session; others fall back to the saved preference. */
  const active = ref<Record<string, string | null>>({});
  /** Follow-up prompts are held per chat for this app session. */
  const queued = ref<Record<string, Record<string, QueuedPrompt[]>>>({});
  /** A stopped or failed queue stays paused while its panel is closed. */
  const queuePaused = ref<Record<string, Record<string, boolean>>>({});
  const backgroundSending = new Set<string>();
  /** Keep a just-posted turn running until its turn event, despite older list responses. */
  const pendingSend = new Set<string>();
  const chatKey = (projectId: string, chatId: string) => `${projectId}/${chatId}`;

  function runningChatIds(projectId: string): string[] {
    const ids = Object.entries(runningChats.value[projectId] ?? {})
      .filter(([, active]) => active).map(([id]) => id);
    const legacy = running.value[projectId];
    if (legacy && !ids.includes(legacy)) ids.push(legacy);
    return ids;
  }

  function isChatRunning(projectId: string, chatId: string): boolean {
    return runningChatIds(projectId).includes(chatId);
  }

  function setChatRunning(projectId: string, chatId: string, active: boolean) {
    (runningChats.value[projectId] ??= {})[chatId] = active || pendingSend.has(chatKey(projectId, chatId));
    running.value[projectId] = Object.entries(runningChats.value[projectId]!)
      .find(([, value]) => value)?.[0] ?? null;
  }

  function markChatSending(projectId: string, chatId: string) {
    pendingSend.add(chatKey(projectId, chatId));
    setChatRunning(projectId, chatId, true);
  }

  function clearChatSending(projectId: string, chatId: string) {
    pendingSend.delete(chatKey(projectId, chatId));
    setChatRunning(projectId, chatId, false);
  }

  function isQueuePaused(projectId: string, chatId: string): boolean {
    return !!queuePaused.value[projectId]?.[chatId];
  }

  function setQueuePaused(projectId: string, chatId: string, paused: boolean) {
    (queuePaused.value[projectId] ??= {})[chatId] = paused;
  }

  function queueFor(projectId: string, chatId: string): QueuedPrompt[] {
    return (queued.value[projectId] ??= {})[chatId] ??= [];
  }

  function enqueue(projectId: string, chatId: string, body: string, preview: string) {
    queueFor(projectId, chatId).push({ id: crypto.randomUUID(), body, preview });
  }

  function removeQueued(projectId: string, chatId: string, id: string) {
    const items = queueFor(projectId, chatId);
    const index = items.findIndex(item => item.id === id);
    if (index >= 0) items.splice(index, 1);
  }

  function prioritizeQueued(projectId: string, chatId: string, id: string) {
    const items = queueFor(projectId, chatId);
    const index = items.findIndex(item => item.id === id);
    if (index > 0) items.unshift(items.splice(index, 1)[0]!);
  }

  function restoreQueued(projectId: string, chatId: string, item: QueuedPrompt) {
    queueFor(projectId, chatId).unshift(item);
  }

  function moveQueued(projectId: string, from: string, to: string) {
    if (from === to) return;
    const items = queued.value[projectId]?.[from];
    if (items?.length) queueFor(projectId, to).push(...items);
    if (queued.value[projectId]) delete queued.value[projectId][from];
    if (isQueuePaused(projectId, from)) setQueuePaused(projectId, to, true);
    if (queuePaused.value[projectId]) delete queuePaused.value[projectId][from];
  }

  function activeChatId(projectId: string): string | null {
    return projectId in active.value ? active.value[projectId]! : readPref(activeKey(projectId));
  }

  function setActive(projectId: string, chatId: string) {
    const previous = activeChatId(projectId);
    active.value[projectId] = chatId;
    writePref(activeKey(projectId), chatId);
    if (previous && previous !== chatId) void drainBackground(projectId, previous);
  }

  /** Record a turn starting or ending locally, without waiting for the next list refresh. */
  function setRunning(projectId: string, chatId: string | null) {
    if (chatId) setChatRunning(projectId, chatId, true);
    else {
      const current = activeChatId(projectId);
      if (current) setChatRunning(projectId, current, false);
    }
  }

  function newChat(projectId: string): string {
    const id = crypto.randomUUID();
    setActive(projectId, id);
    return id;
  }

  async function load(projectId: string) {
    try {
      const res = await daemon.api.listChats(projectId);
      chats.value[projectId] = res.chats;
      runningChats.value[projectId] = Object.fromEntries(res.chats
        .filter(chat => chat.running || chat.id === res.running)
        .map(chat => [chat.id, true]));
      for (const key of pendingSend) {
        if (key.startsWith(`${projectId}/`)) runningChats.value[projectId]![key.slice(projectId.length + 1)] = true;
      }
      running.value[projectId] = null;
      running.value[projectId] = runningChatIds(projectId)[0] ?? null;
    } catch {
      // An older daemon, or a transient failure: keep what we have.
    }
  }

  /** Delete a chat; the active one is replaced by a new chat. Throws when the daemon refuses. */
  async function remove(projectId: string, chatId: string) {
    await daemon.api.deleteChat(projectId, chatId);
    if (queued.value[projectId]) delete queued.value[projectId][chatId];
    if (queuePaused.value[projectId]) delete queuePaused.value[projectId][chatId];
    if (runningChats.value[projectId]) delete runningChats.value[projectId][chatId];
    pendingSend.delete(chatKey(projectId, chatId));
    if (running.value[projectId] === chatId) {
      running.value[projectId] = null;
      running.value[projectId] = runningChatIds(projectId)[0] ?? null;
    }
    chats.value[projectId] = (chats.value[projectId] ?? []).filter((c) => c.id !== chatId);
    if (activeChatId(projectId) === chatId) newChat(projectId);
  }

  async function drainBackground(projectId: string, chatId: string) {
    const key = `${projectId}/${chatId}`;
    if (activeChatId(projectId) === chatId || isChatRunning(projectId, chatId)
      || isQueuePaused(projectId, chatId)
      || !queued.value[projectId]?.[chatId]?.length || backgroundSending.has(key)) return;
    backgroundSending.add(key);
    let sent = false;
    try {
      const history = await daemon.api.chatHistory(projectId, chatId);
      if (history.running || activeChatId(projectId) === chatId) return;
      const next = queued.value[projectId]?.[chatId]?.[0];
      if (!next || isQueuePaused(projectId, chatId)) return;
      removeQueued(projectId, chatId, next.id);
      try {
        markChatSending(projectId, chatId);
        await daemon.api.sendChatMessage(projectId, chatId, next.body);
        sent = true;
      } catch {
        clearChatSending(projectId, chatId);
        restoreQueued(projectId, chatId, next);
        setQueuePaused(projectId, chatId, true);
      }
    } catch {
      // A transient history failure leaves the queue intact for a later turn or reconnect.
    } finally {
      backgroundSending.delete(key);
      // A short turn may end before its message POST resolves.
      if (sent && queued.value[projectId]?.[chatId]?.length && !isChatRunning(projectId, chatId)) {
        void drainBackground(projectId, chatId);
      }
    }
  }

  const reloadAll = () => Object.keys(chats.value).forEach((id) => {
    void load(id).then(() => {
      for (const chatId of Object.keys(queued.value[id] ?? {})) void drainBackground(id, chatId);
    });
  });

  const offs = [
    daemon.onReconnected(() => {
      pendingSend.clear();
      reloadAll();
    }),
    daemon.on("events.lagged", reloadAll),
    daemon.on("chat.turn", (e) => {
      clearChatSending(e.projectId, e.chatId);
      void drainBackground(e.projectId, e.chatId);
      if (e.projectId in chats.value) void load(e.projectId);
    }),
  ];
  onScopeDispose(() => offs.forEach((off) => off()));

  return { chats, running, queued, enqueue, removeQueued, prioritizeQueued, restoreQueued, moveQueued, isQueuePaused, setQueuePaused, runningChatIds, isChatRunning, setChatRunning, markChatSending, clearChatSending, activeChatId, setActive, setRunning, newChat, load, remove };
});
