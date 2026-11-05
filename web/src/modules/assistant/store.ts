import { defineStore } from "pinia";
import { computed, onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type {
  ProviderSignInState,
  RuntimePhase,
  RuntimeStatus,
  SetupProvider,
  SetupRuntimeEvent,
  SetupSnapshot,
  SetupStatus,
} from "@/core/types";
import { errorMessage } from "@orso/utils";
import { buildRows, orderProviders, siteBuilderRow } from "./rows";

const PHASES: readonly RuntimePhase[] = [
  "notDownloaded",
  "downloading",
  "finishing",
  "ready",
  "downloadError",
  "updateAvailable",
];

const EMPTY_RUNTIME: RuntimeStatus = {
  phase: "notDownloaded",
  version: null,
  source: null,
  progress: null,
  message: null,
};

/** The node + assistant state both setup endpoints describe. */
export interface AssistantSnapshot {
  node: RuntimeStatus | null;
  providers: SetupProvider[];
}

function isRuntimeStatus(value: unknown): value is RuntimeStatus {
  return (
    typeof value === "object" &&
    value !== null &&
    "phase" in value &&
    PHASES.includes((value as RuntimeStatus).phase)
  );
}

/**
 * Fold the runtime snapshot into one status entry. Names and older fields stay
 * from `/status`; download and readiness fields come from `/runtimes` when it
 * sends them. `signedIn: null` and `freeModels: false` are meaningful values,
 * so they win over the status copy.
 */
function mergeProvider(base: SetupProvider, update: SetupProvider): SetupProvider {
  return {
    ...base,
    name: base.name || update.name,
    available: base.available ?? update.available,
    runtime: update.runtime ?? base.runtime,
    installed: update.installed ?? base.installed,
    signedIn: update.signedIn !== undefined ? update.signedIn : base.signedIn,
    chatReady: update.chatReady ?? base.chatReady,
    freeModels: update.freeModels ?? base.freeModels,
    reason: update.reason !== undefined ? update.reason : base.reason,
  };
}

/**
 * Fold `GET /api/setup/runtimes` into `GET /api/setup/status`. The runtime
 * snapshot wins for download state; the status keeps names and older fields.
 * A daemon without `/runtimes` still yields a usable snapshot from `/status`.
 */
function mergeSnapshot(status: SetupStatus, runtimes: SetupSnapshot | null): AssistantSnapshot {
  if (!runtimes) return { node: status.node ?? null, providers: status.providers ?? [] };
  const extra = new Map((runtimes.providers ?? []).map((p) => [p.id, p] as const));
  const providers = (status.providers ?? []).map((provider) => {
    const runtime = extra.get(provider.id);
    return runtime ? mergeProvider(provider, runtime) : provider;
  });
  const known = new Set(providers.map((p) => p.id));
  for (const provider of runtimes.providers ?? []) {
    if (!known.has(provider.id)) providers.push(provider);
  }
  return { node: runtimes.node ?? status.node ?? null, providers };
}

/**
 * Assistant setup: download state for the assistants and the site builder,
 * plus the per-provider sign-in flows. Actions are optimistic so a row reacts
 * on click; daemon events (`setup.runtime`) then correct the snapshot.
 */
export const useAssistantStore = defineStore("assistant", () => {
  const daemon = useDaemon();

  const snapshot = ref<AssistantSnapshot | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);
  /** Sign-in flows started this session, by provider id. */
  const signIns = ref<Record<string, ProviderSignInState | null>>({});

  const node = computed<RuntimeStatus | null>(() => snapshot.value?.node ?? null);
  const providers = computed<SetupProvider[]>(() => snapshot.value?.providers ?? []);
  /** Providers that can open a chat right now, in display order. */
  const chatReadyProviders = computed(() =>
    orderProviders(providers.value.filter((p) => p.chatReady === true)),
  );
  /** OpenCode is the recommendation; fall back to the first ready assistant. */
  const recommended = computed<SetupProvider | null>(
    () =>
      providers.value.find((p) => p.id === "opencode") ??
      chatReadyProviders.value[0] ??
      providers.value[0] ??
      null,
  );
  const rows = computed(() => buildRows(providers.value));
  const siteBuilder = computed(() => siteBuilderRow(node.value));

  function runtimeFor(id: string): RuntimeStatus | null {
    if (id === "node") return snapshot.value?.node ?? null;
    return snapshot.value?.providers.find((p) => p.id === id)?.runtime ?? null;
  }

  function mergeRuntime(
    current: RuntimeStatus | null | undefined,
    patch: Partial<RuntimeStatus>,
  ): RuntimeStatus {
    return { ...EMPTY_RUNTIME, ...current, ...patch };
  }

  /** Replace runtime fields for `node` or one assistant, keeping event-only fields. */
  function patchRuntime(id: string, patch: Partial<RuntimeStatus>) {
    const current = snapshot.value;
    if (!current) return;
    if (id === "node") {
      snapshot.value = { ...current, node: mergeRuntime(current.node, patch) };
      return;
    }
    snapshot.value = {
      ...current,
      providers: current.providers.map((provider) => {
        if (provider.id !== id) return provider;
        const runtime = mergeRuntime(provider.runtime, patch);
        const installed =
          runtime.phase === "ready" || runtime.phase === "updateAvailable"
            ? true
            : provider.installed;
        return { ...provider, runtime, installed };
      }),
    };
  }

  function patchProvider(id: string, patch: Partial<SetupProvider>) {
    const current = snapshot.value;
    if (!current) return;
    snapshot.value = {
      ...current,
      providers: current.providers.map((provider) =>
        provider.id === id ? { ...provider, ...patch } : provider,
      ),
    };
  }

  async function load() {
    loading.value = true;
    error.value = null;
    try {
      const [status, runtimes] = await Promise.all([
        daemon.api.setupStatus(),
        // An older daemon has no runtime snapshot; the status alone is enough.
        Promise.resolve()
          .then(() => daemon.api.setupRuntimes())
          .catch(() => null as SetupSnapshot | null),
      ]);
      snapshot.value = mergeSnapshot(status, runtimes);
    } catch (e) {
      error.value = errorMessage(e);
    } finally {
      loading.value = false;
    }
  }

  /** Start a download, or retry a failed one; the row reacts before the POST resolves. */
  async function runDownload(id: string, call: () => Promise<RuntimeStatus>) {
    patchRuntime(id, { phase: "downloading", progress: 0, message: null });
    try {
      const result = await call();
      if (isRuntimeStatus(result)) patchRuntime(id, result);
    } catch (e) {
      patchRuntime(id, { phase: "downloadError", progress: null, message: errorMessage(e) });
    }
  }

  async function download(id: string) {
    await runDownload(id, () => daemon.api.downloadRuntime(id));
  }

  async function retry(id: string) {
    await runDownload(id, () => daemon.api.retryRuntime(id));
  }

  /** Stop a download; the daemon restores the previous ready copy, if any. */
  async function cancel(id: string) {
    const current = runtimeFor(id);
    // A ready/update copy has a version; a first download has nothing to fall back to.
    const hadCopy =
      current?.version != null || current?.phase === "ready" || current?.phase === "updateAvailable";
    patchRuntime(id, {
      phase: hadCopy ? "ready" : "notDownloaded",
      progress: null,
      message: null,
    });
    try {
      const result = await daemon.api.cancelRuntime(id);
      if (isRuntimeStatus(result)) patchRuntime(id, result);
    } catch (e) {
      // Keep the optimistic state; the next snapshot or event corrects it.
      patchRuntime(id, { message: errorMessage(e) });
    }
  }

  /** Check that an assistant can actually start a chat. */
  async function verify(id: string) {
    try {
      const result = await daemon.api.verifyProvider(id);
      patchProvider(id, { chatReady: result.chatReady, reason: result.reason ?? null });
    } catch (e) {
      const message = errorMessage(e);
      patchProvider(id, { chatReady: false, reason: message });
      error.value = message;
    }
  }

  function setSignIn(id: string, state: ProviderSignInState | null) {
    signIns.value = { ...signIns.value, [id]: state };
  }

  function signInFor(id: string): ProviderSignInState | null {
    return signIns.value[id] ?? null;
  }

  function clearSignIn(id: string) {
    const next = { ...signIns.value };
    delete next[id];
    signIns.value = next;
  }

  async function startSignIn(id: string, call: () => Promise<ProviderSignInState>) {
    try {
      const state = await call();
      setSignIn(id, state);
      if (state.state === "connected") patchProvider(id, { signedIn: true, reason: null });
      if (state.state === "failed") error.value = state.message ?? "Sign-in failed";
    } catch (e) {
      const message = errorMessage(e);
      setSignIn(id, { state: "failed", message });
      error.value = message;
    }
  }

  async function signIn(id: string) {
    await startSignIn(id, () => daemon.api.signInProvider(id));
  }

  async function submitCode(id: string, code: string) {
    await startSignIn(id, () => daemon.api.submitProviderSignInCode(id, code));
  }

  async function cancelSignIn(id: string) {
    try {
      const state = await daemon.api.cancelProviderSignIn(id);
      setSignIn(id, state);
    } catch (e) {
      const message = errorMessage(e);
      setSignIn(id, { state: "failed", message });
      error.value = message;
    }
  }

  async function signOut(id: string) {
    try {
      await daemon.api.signOutProvider(id);
      const provider = providers.value.find((p) => p.id === id);
      patchProvider(id, {
        signedIn: false,
        // OpenCode keeps working on free models; the others need a sign-in.
        chatReady: provider?.freeModels ? provider.chatReady : false,
        reason: null,
      });
      clearSignIn(id);
    } catch (e) {
      error.value = errorMessage(e);
    }
  }

  /** Daemon-authoritative readiness (adapters, sign-in) needs a status refresh. */
  const refreshTimers = new Set<ReturnType<typeof setTimeout>>();

  function scheduleRefresh(delay = 750) {
    const timer = setTimeout(() => {
      refreshTimers.delete(timer);
      void load();
    }, delay);
    refreshTimers.add(timer);
  }

  /** Merge one live download event; fields the event omits keep their snapshot value. */
  function applyRuntimeEvent(event: SetupRuntimeEvent) {
    if (event.runtime !== "node" && !providers.value.some((p) => p.id === event.runtime)) return;
    const patch: Partial<RuntimeStatus> = { phase: event.phase };
    if (event.version !== undefined) patch.version = event.version;
    if (event.source !== undefined) patch.source = event.source;
    if (event.progress !== undefined) patch.progress = event.progress;
    if (event.message !== undefined) patch.message = event.message;
    patchRuntime(event.runtime, patch);

    if (event.runtime !== "node" && event.phase === "ready") {
      // OpenCode runs free models, so its runtime is enough to chat. The
      // others may still be installing their adapter: ask the daemon again.
      const provider = providers.value.find((p) => p.id === event.runtime);
      if (provider?.freeModels) patchProvider(event.runtime, { chatReady: true, reason: null });
      scheduleRefresh(provider?.freeModels ? 250 : 1_500);
    }
  }

  const offs = [
    daemon.on("setup.runtime", applyRuntimeEvent),
    daemon.onReconnected(() => void load()),
    daemon.on("events.lagged", () => void load()),
  ];
  onScopeDispose(() => {
    offs.forEach((off) => off());
    for (const timer of refreshTimers) clearTimeout(timer);
    refreshTimers.clear();
  });

  return {
    snapshot,
    loading,
    error,
    signIns,
    node,
    providers,
    chatReadyProviders,
    recommended,
    rows,
    siteBuilder,
    load,
    download,
    retry,
    cancel,
    verify,
    signIn,
    submitCode,
    cancelSignIn,
    signOut,
    signInFor,
    clearSignIn,
  };
});
