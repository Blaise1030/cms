import { defineStore } from "pinia";
import { ref } from "vue";
import { useDaemon } from "@/core/daemon";
import { errorMessage } from "@/lib/errors";
import { readPref, writePref } from "@/lib/storage";

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

  /** Start the chosen provider's session; resolves `true` on success. */
  async function start(projectId: string): Promise<boolean> {
    if (!provider.value) {
      error.value = "No terminal providers are configured on the daemon";
      return false;
    }
    starting.value = true;
    error.value = null;
    try {
      await daemon.api.startTerminal(projectId, provider.value);
      writePref(PROVIDER_KEY, provider.value);
      return true;
    } catch (e) {
      error.value = errorMessage(e);
      return false;
    } finally {
      starting.value = false;
    }
  }

  return { providers, provider, error, starting, setProvider, loadProviders, start };
});
