// Pure row model for assistant setup. No Vue and no daemon calls, so the state
// machine, ordering and copy are easy to test (and reusable by the setup UI).
//
// Copy rules: never show "agent", "CLI", "ACP", "SDK", "npx", "PATH" or
// "runtime" to the user. Keep the product words: assistant, site, download.

import type { RuntimePhase, RuntimeSource, RuntimeStatus, SetupProvider } from "@/core/types";

/** Fixed order: the recommendation first, then the plan-backed assistants. */
export const PROVIDER_ORDER = ["opencode", "codex", "claude", "cursor"] as const;

/** Fallback names when a daemon (or a test) does not send one. */
export const PROVIDER_NAMES: Record<string, string> = {
  opencode: "OpenCode",
  codex: "Codex",
  claude: "Claude",
  cursor: "Cursor",
  node: "Site builder",
};

/** What each row says about the account it needs (or what it is for). */
export const PROVIDER_DESCRIPTIONS: Record<string, string> = {
  opencode: "Free models, no account needed.",
  codex: "Uses your ChatGPT plan.",
  claude: "Uses your Claude plan.",
  cursor: "Uses your Cursor plan.",
  node: "Lets orso build and preview your site.",
};

/** The exact user-facing labels, kept together so tests can assert on them. */
export const LABELS = {
  download: "Download",
  downloading: "Downloading",
  cancel: "Cancel",
  settingUp: "Setting up…",
  retry: "Retry",
  update: "Update",
  connect: "Connect",
  signIn: "Sign in",
  connected: "Connected",
  signInRequired: "Sign-in required",
  downloadFailed: "Download failed",
  updateAvailable: "Update available",
} as const;

export type RowTone = "success" | "warning" | "destructive" | "muted";

/** The one action a row offers; `none` while there is nothing to press. */
export type RowAction =
  | "download"
  | "cancel"
  | "retry"
  | "update"
  | "connect"
  | "signIn"
  | "none";

export interface AssistantRow {
  id: string;
  name: string;
  /** Account requirement, or what the site builder is for. */
  description: string;
  recommended: boolean;
  phase: RuntimePhase;
  version: string | null;
  source: RuntimeSource | null;
  /** 0–100 while downloading, else null. */
  progress: number | null;
  message: string | null;
  /** The headline under the name: Connected, Sign-in required, Download failed, … */
  status: string;
  action: RowAction;
  actionLabel: string;
  tone: RowTone;
  signInRequired: boolean;
  chatReady: boolean;
  freeModels: boolean;
  provider: SetupProvider;
}

const EMPTY_RUNTIME: RuntimeStatus = {
  phase: "notDownloaded",
  version: null,
  source: null,
  progress: null,
  message: null,
};

/** The runtime a provider reports; older daemons only send `available`/`installed`. */
export function runtimeOf(provider: SetupProvider): RuntimeStatus {
  if (provider.runtime) return provider.runtime;
  if (provider.installed || provider.available) {
    return { ...EMPTY_RUNTIME, phase: "ready", source: "system" };
  }
  return EMPTY_RUNTIME;
}

/** Round and clamp a download percentage so a bad event cannot render "140%". */
export function clampProgress(progress: number | null | undefined): number | null {
  if (progress == null || !Number.isFinite(progress)) return null;
  return Math.min(100, Math.max(0, Math.round(progress)));
}

/**
 * Rank a provider for display. Unknown ids keep their daemon order after the
 * known ones. The rank depends only on the id, never on its state: a row never
 * jumps while its download, sign-in or connect state changes.
 */
function rank(id: string): number {
  const index = (PROVIDER_ORDER as readonly string[]).indexOf(id);
  return index < 0 ? PROVIDER_ORDER.length : index;
}

export function orderProviders(providers: readonly SetupProvider[]): SetupProvider[] {
  return providers
    .map((provider, index) => ({ provider, index }))
    .sort((a, b) => rank(a.provider.id) - rank(b.provider.id) || a.index - b.index)
    .map(({ provider }) => provider);
}

/** Derive one row's phase, labels and action. */
export function deriveRow(provider: SetupProvider): AssistantRow {
  const runtime = runtimeOf(provider);
  const phase = runtime.phase;
  const progress = phase === "downloading" ? clampProgress(runtime.progress) : null;
  const signedIn = provider.signedIn ?? null;
  const freeModels = provider.freeModels === true;
  const chatReady = provider.chatReady ?? (phase === "ready" || phase === "updateAvailable");
  const signInRequired = phase === "ready" && signedIn === false && !freeModels;

  const base: Omit<AssistantRow, "status" | "action" | "actionLabel" | "tone"> = {
    id: provider.id,
    name: provider.name || PROVIDER_NAMES[provider.id] || provider.id,
    description: PROVIDER_DESCRIPTIONS[provider.id] ?? "",
    recommended: provider.id === "opencode",
    phase,
    version: runtime.version,
    source: runtime.source,
    progress,
    message: runtime.message,
    signInRequired,
    chatReady,
    freeModels,
    provider,
  };

  switch (phase) {
    case "notDownloaded":
      return { ...base, status: "", action: "download", actionLabel: LABELS.download, tone: "muted" };
    case "downloading":
      return {
        ...base,
        status: progress == null ? LABELS.downloading : `${progress}%`,
        action: "cancel",
        actionLabel: LABELS.cancel,
        tone: "muted",
      };
    case "finishing":
      return { ...base, status: LABELS.settingUp, action: "none", actionLabel: "", tone: "muted" };
    case "downloadError":
      return {
        ...base,
        status: LABELS.downloadFailed,
        action: "retry",
        actionLabel: LABELS.retry,
        tone: "destructive",
      };
    case "updateAvailable":
      return {
        ...base,
        status: LABELS.updateAvailable,
        action: "update",
        actionLabel: LABELS.update,
        tone: "warning",
      };
    case "ready":
      // The site builder is never signed in; when it is ready there is nothing to press.
      if (provider.id === "node") {
        return { ...base, status: LABELS.connected, action: "none", actionLabel: "", tone: "success" };
      }
      if (signInRequired || (!chatReady && signedIn !== true)) {
        return {
          ...base,
          status: LABELS.signInRequired,
          action: "signIn",
          actionLabel: LABELS.signIn,
          tone: "warning",
        };
      }
      // OpenCode works signed out; signing in is only for extra models.
      if (freeModels && signedIn !== true) {
        return {
          ...base,
          status: LABELS.connected,
          action: "signIn",
          actionLabel: LABELS.signIn,
          tone: "success",
        };
      }
      return { ...base, status: LABELS.connected, action: "connect", actionLabel: LABELS.connect, tone: "success" };
  }
}

/** The assistant rows, in the fixed order. */
export function buildRows(providers: readonly SetupProvider[]): AssistantRow[] {
  return orderProviders(providers).map(deriveRow);
}

/** The separate "Site builder" row for the Node download. */
export function siteBuilderRow(node: RuntimeStatus | null): AssistantRow {
  const ready = node?.phase === "ready" || node?.phase === "updateAvailable";
  return deriveRow({
    id: "node",
    name: PROVIDER_NAMES.node!,
    available: ready,
    runtime: node ?? undefined,
    installed: ready,
    signedIn: null,
    chatReady: ready,
    freeModels: false,
    reason: null,
  });
}
