import { defineStore } from "pinia";
import { ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { TerminalSession } from "@/core/types";
import { errorMessage, readPref, writePref } from "@workbench/utils";

const PROVIDER_KEY = "workbench.terminal.provider";

export interface TerminalProvider {
  id: string;
  command: string[];
}

export const useTerminalStore = defineStore("terminal", () => {
  const daemon = useDaemon();

  const providers = ref<TerminalProvider[]>([]);
  const provider = ref<string | null>(readPref(PROVIDER_KEY));
  const error = ref<string | null>(null);
  const starting = ref(false);
  /** Past and live sessions of the panel's project, most recently started first. */
  const sessions = ref<TerminalSession[]>([]);
  /** The live session's id, when one is running. */
  const sessionId = ref<string | null>(null);
  /** The panel is popped out of its column into a draggable window. */
  const floating = ref(false);

  function setProvider(id: string) {
    provider.value = id;
    writePref(PROVIDER_KEY, id);
  }

  async function loadProviders() {
    try {
      providers.value = await daemon.api.terminalProviders();
    } catch (e) {
      error.value = errorMessage(e);
      return;
    }
    if (!providers.value.some((p) => p.id === provider.value)) {
      provider.value = providers.value[0]?.id ?? null;
    }
  }

  /**
   * Start the chosen provider's session, or `resume` a past one (in its own provider).
   * Resolves `true` on success.
   */
  async function start(projectId: string, resume?: TerminalSession): Promise<boolean> {
    const chosen = resume?.provider ?? provider.value;
    if (!chosen) {
      error.value = "No terminal providers are configured on the daemon";
      return false;
    }
    starting.value = true;
    error.value = null;
    try {
      const res = resume
        ? await daemon.api.startTerminal(projectId, chosen, resume.id)
        : await daemon.api.startTerminal(projectId, chosen);
      sessionId.value = res.sessionId ?? null;
      if (!resume) writePref(PROVIDER_KEY, chosen);
      return true;
    } catch (e) {
      error.value = errorMessage(e);
      return false;
    } finally {
      starting.value = false;
    }
  }

  async function loadSessions(projectId: string) {
    try {
      sessions.value = await daemon.api.terminalSessions(projectId);
    } catch {
      // An older daemon, or a transient failure: keep what we have.
    }
  }

  return {
    providers,
    provider,
    error,
    starting,
    sessions,
    sessionId,
    floating,
    setProvider,
    loadProviders,
    loadSessions,
    start,
  };
});
