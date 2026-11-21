import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import { useAssistantStore } from "@/modules/assistant/store";
import { readPref, writePref } from "@orso/utils";

/** Assistants this UI can drive. */
export const CHAT_PROVIDERS = ["cursor", "claude", "codex", "opencode"];

const PROVIDER_KEY = "orso.chat.provider";
/** The pick lived here when agents ran in Terminal; read it once so nobody loses their choice. */
const LEGACY_PROVIDER_KEY = "orso.terminal.provider";
const modelKey = (provider: string) => `orso.chat.model.${provider}`;

export interface AgentProvider {
  id: string;
  command: string[];
}

export interface AgentModel {
  id: string;
  name: string;
}

/** The agent CLIs the daemon offers, their models, and the user's picks. */
export const useAgentProvidersStore = defineStore("agentProviders", () => {
  const daemon = useDaemon();
  const assistant = useAssistantStore();

  const providers = ref<AgentProvider[]>([]);
  const provider = ref<string | null>(readPref(PROVIDER_KEY) ?? readPref(LEGACY_PROVIDER_KEY));

  /**
   * The assistants a chat can start with now, in the user's order. The setup
   * snapshot says which built-in ones are ready; until it lands, every provider the
   * daemon lists. Providers the setup check doesn't know (custom ones from the agents
   * config) follow, in the daemon's order, so choosing one still works.
   */
  const ready = computed(() => {
    const listed = providers.value.map((p) => p.id);
    const builtIn = assistant.snapshot
      ? assistant.chatReadyProviders.map((p) => p.id).filter((id) => CHAT_PROVIDERS.includes(id))
      : assistant.ordered(listed.filter((id) => CHAT_PROVIDERS.includes(id)));
    const custom = listed.filter((id) => !CHAT_PROVIDERS.includes(id));
    return [...builtIn, ...custom];
  });

  /**
   * The assistant a new chat (or a sync) starts with. Once the user has ordered
   * the assistants in App settings, that is the first ready one in their order;
   * before that, the remembered pick while it is ready, else the first ready one.
   */
  const preferred = computed<string | null>(() => {
    // Nothing known yet: the remembered pick, as before.
    if (!assistant.snapshot && !providers.value.length) return provider.value;
    const ids = ready.value;
    if (!assistant.order && provider.value && ids.includes(provider.value)) return provider.value;
    return ids[0] ?? null;
  });

  /** Per provider; absent until asked, empty when the daemon couldn't list them. */
  const models = ref<Record<string, AgentModel[]>>({});
  /** Model picks made this session, by provider; `null` is Default. */
  const picks = ref<Record<string, string | null>>({});
  const loaded = new Set<string>();
  const loading = new Map<string, Promise<void>>();

  function setProvider(id: string) {
    provider.value = id;
    writePref(PROVIDER_KEY, id);
  }

  async function loadProviders() {
    try {
      providers.value = await daemon.api.chatProviders();
    } catch {
      // An older daemon, or a transient failure: keep the current pick.
      return;
    }
    if (!providers.value.some((p) => p.id === provider.value)) {
      provider.value = providers.value[0]?.id ?? null;
    }
  }

  function loadModels(id: string): Promise<void> {
    if (loaded.has(id)) return Promise.resolve();
    const pending =
      loading.get(id) ??
      (async () => {
        try {
          models.value[id] = await daemon.api.agentModels(id);
          loaded.add(id);
        } catch {
          // Offer Default only; the next load asks again.
          models.value[id] = [];
        } finally {
          loading.delete(id);
        }
      })();
    loading.set(id, pending);
    return pending;
  }

  function modelPick(id: string): string | null {
    return id in picks.value ? picks.value[id]! : readPref(modelKey(id));
  }

  function setModel(id: string, model: string | null) {
    picks.value[id] = model;
    writePref(modelKey(id), model);
  }

  return {
    providers,
    provider,
    ready,
    preferred,
    models,
    setProvider,
    loadProviders,
    loadModels,
    modelPick,
    setModel,
  };
});
