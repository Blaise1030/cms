import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { ChatSummary } from "@/core/types";
import { readPref, writePref } from "@workbench/utils";

const activeKey = (projectId: string) => `workbench.chat.active.${projectId}`;

export const useChatStore = defineStore("chat", () => {
  const daemon = useDaemon();

  /** Each loaded project's chats, as the daemon lists them. */
  const chats = ref<Record<string, ChatSummary[]>>({});
  /** The chat with a turn in flight, per loaded project. */
  const running = ref<Record<string, string | null>>({});
  /** Active chat ids chosen this session; others fall back to the saved preference. */
  const active = ref<Record<string, string | null>>({});

  function activeChatId(projectId: string): string | null {
    return projectId in active.value ? active.value[projectId]! : readPref(activeKey(projectId));
  }

  function setActive(projectId: string, chatId: string) {
    active.value[projectId] = chatId;
    writePref(activeKey(projectId), chatId);
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
      running.value[projectId] = res.running;
    } catch {
      // An older daemon, or a transient failure: keep what we have.
    }
  }

  /** Delete a chat; the active one is replaced by a new chat. Throws when the daemon refuses. */
  async function remove(projectId: string, chatId: string) {
    await daemon.api.deleteChat(projectId, chatId);
    chats.value[projectId] = (chats.value[projectId] ?? []).filter((c) => c.id !== chatId);
    if (activeChatId(projectId) === chatId) newChat(projectId);
  }

  const reloadAll = () => Object.keys(chats.value).forEach((id) => void load(id));

  const offs = [
    daemon.onReconnected(reloadAll),
    daemon.on("events.lagged", reloadAll),
    daemon.on("chat.turn", (e) => {
      if (e.projectId in chats.value) void load(e.projectId);
    }),
    // The terminal and chat tabs share one conversation: follow the terminal's session.
    daemon.on("terminal.session", (e) => setActive(e.projectId, e.sessionId)),
  ];
  onScopeDispose(() => offs.forEach((off) => off()));

  return { chats, running, activeChatId, setActive, newChat, load, remove };
});
