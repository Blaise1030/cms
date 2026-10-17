import { defineStore } from "pinia";
import { ref } from "vue";
import { useDaemon } from "@/core/daemon";
import { readPref, writePref } from "@workbench/utils";

/** Named for the old Terminal tab; kept so users keep their pick. */
const PROVIDER_KEY = "workbench.terminal.provider";

export interface AgentProvider {
  id: string;
  command: string[];
}

/** The agent CLIs the daemon offers, and the user's pick. */
export const useAgentProvidersStore = defineStore("agentProviders", () => {
  const daemon = useDaemon();

  const providers = ref<AgentProvider[]>([]);
  const provider = ref<string | null>(readPref(PROVIDER_KEY));

  function setProvider(id: string) {
    provider.value = id;
    writePref(PROVIDER_KEY, id);
  }

  async function loadProviders() {
    try {
      providers.value = await daemon.api.terminalProviders();
    } catch {
      // An older daemon, or a transient failure: keep the current pick.
      return;
    }
    if (!providers.value.some((p) => p.id === provider.value)) {
      provider.value = providers.value[0]?.id ?? null;
    }
  }

  return { providers, provider, setProvider, loadProviders };
});
