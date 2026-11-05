<script setup lang="ts">
// First-run setup, in three short steps: pick an assistant, add the site
// builder, then optionally connect GitHub. The wizard keeps the component name
// (and the `continue` event) used by App.vue, but replaces the old single card.
//
// Copy rules: never show "agent", "CLI", "ACP", "SDK", "npx", "PATH" or
// "runtime" to the user. Keep the product words: assistant, site, download.
import { computed, onMounted, ref, watch } from "vue";
import {
  Check,
  CircleAlert,
  CircleCheck,
  Download,
  ExternalLink,
  LogIn,
  Package,
  RefreshCw,
  RotateCcw,
  X,
} from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { useDaemon } from "@/core/daemon";
import type { ProviderSignInState, RuntimeStatus, SetupProvider, SetupStatus } from "@/core/types";
import PawMark from "@/app/PawMark.vue";
import ProviderIcon from "@/modules/chat/ProviderIcon.vue";
import {
  PROVIDER_NAMES,
  buildRows,
  deriveRow,
  type AssistantRow,
} from "@/modules/assistant/rows";
import { formatDownloadSize, sourceLabel } from "@/modules/assistant/presentation";
import SignInPanel from "@/modules/assistant/SignInPanel.vue";
import { useAssistantStore } from "@/modules/assistant/store";
import DeviceCodeDialog from "@/modules/github/DeviceCodeDialog.vue";
import { useGithubStore } from "@/modules/github/store";

/** `initial`: a status the caller already fetched; without it the check fetches its own. */
const props = defineProps<{ initial?: SetupStatus }>();
const emit = defineEmits<{ continue: [] }>();

const daemon = useDaemon();
const gh = useGithubStore();
const assistant = useAssistantStore();

type Step = 1 | 2 | 3;

const STEPS: readonly { n: Step; label: string }[] = [
  { n: 1, label: "Assistant" },
  { n: 2, label: "Site builder" },
  { n: 3, label: "GitHub" },
];

/** The card list order; OpenCode is the recommendation and the default. */
const CARD_ORDER = ["opencode", "claude", "codex", "cursor"] as const;

const EMPTY_RUNTIME: RuntimeStatus = {
  phase: "notDownloaded",
  version: null,
  source: null,
  progress: null,
  message: null,
};

const step = ref<Step>(1);
const selectedId = ref("opencode");
const status = ref<SetupStatus | null>(props.initial ?? null);
const checking = ref(false);
const failed = ref(false);

/** Git and the GitHub account stay system tools; the assistant is downloaded here. */
async function recheck() {
  checking.value = true;
  try {
    const [next] = await Promise.all([daemon.api.setupStatus(), assistant.load()]);
    status.value = next;
    failed.value = false;
  } catch {
    failed.value = true;
  } finally {
    checking.value = false;
  }
}

/** Always show four cards; a daemon that does not report one still lets the user try. */
function cardFor(id: string): AssistantRow {
  const provider = assistant.providers.find((p) => p.id === id);
  if (provider) return deriveRow(provider);
  const fallback: SetupProvider = {
    id,
    name: PROVIDER_NAMES[id] ?? id,
    available: false,
    runtime: EMPTY_RUNTIME,
    installed: false,
    signedIn: null,
    chatReady: false,
    freeModels: id === "opencode",
    reason: null,
  };
  return deriveRow(fallback);
}

const cards = computed<AssistantRow[]>(() => CARD_ORDER.map(cardFor));

/** If OpenCode is not reported, the first card acts as the selection. */
const selectedRow = computed<AssistantRow | null>(() => {
  const card = cards.value.find((row) => row.id === selectedId.value);
  if (card) return card;
  const provider = assistant.providers.find((p) => p.id === selectedId.value);
  if (provider) return deriveRow(provider);
  return cards.value[0] ?? null;
});

/** Assistants that are already on the Mac and can chat right now. */
const existingRows = computed<AssistantRow[]>(() =>
  buildRows(
    assistant.providers.filter(
      (provider) => provider.chatReady === true && provider.runtime?.source === "system",
    ),
  ),
);

const signInState = computed<ProviderSignInState | null>(() =>
  selectedRow.value ? panelFor(selectedRow.value.id) : null,
);

/** Never rendered: the `v-if` guards the panel, this only keeps the prop non-null. */
const NO_SIGN_IN: ProviderSignInState = { state: "cancelled" };

/** The sign-in step to show under a card, if one is waiting on the user. */
function panelFor(id: string): ProviderSignInState | null {
  const state = assistant.signInFor(id);
  if (!state) return null;
  return state.state === "browser" || state.state === "code" || state.state === "failed"
    ? state
    : null;
}

function select(id: string) {
  selectedId.value = id;
}

/** The CTA under the card list: set up, sign in, retry, or move on. */
const setupLabel = computed(() => {
  const row = selectedRow.value;
  if (!row) return "Continue";
  if (row.chatReady) return "Continue";
  switch (row.phase) {
    case "downloading":
      return "Downloading…";
    case "finishing":
      return "Setting up…";
    case "downloadError":
      return "Retry";
    case "ready":
      return "Sign in";
    default:
      return `Set up with ${row.name}`;
  }
});

const setupDisabled = computed(() => {
  const row = selectedRow.value;
  return !row || row.phase === "downloading" || row.phase === "finishing";
});

function startSelected() {
  const row = selectedRow.value;
  if (!row) return;
  if (row.chatReady) {
    step.value = 2;
    return;
  }
  switch (row.phase) {
    case "downloadError":
      void assistant.retry(row.id);
      break;
    case "ready":
      void assistant.signIn(row.id);
      break;
    case "downloading":
    case "finishing":
      break;
    default:
      void assistant.download(row.id);
  }
}

function cancelSelected() {
  if (selectedRow.value) void assistant.cancel(selectedRow.value.id);
}

function onSubmitCode(id: string, code: string) {
  void assistant.submitCode(id, code);
}

function onCancelSignIn(id: string) {
  void assistant.cancelSignIn(id);
}

function onRetrySignIn(id: string) {
  void assistant.signIn(id);
}

function onDismissSignIn(id: string) {
  assistant.clearSignIn(id);
}

// Step 2: the site builder.
const site = computed(() => assistant.siteBuilder);
const sitePresent = computed(() => assistant.node != null);
const siteReady = computed(
  () =>
    !sitePresent.value ||
    assistant.node?.phase === "ready" ||
    assistant.node?.phase === "updateAvailable",
);

const siteLabel = computed(() => {
  switch (site.value.phase) {
    case "downloading":
      return "Cancel";
    case "finishing":
      return "Setting up…";
    case "downloadError":
      return "Retry";
    case "ready":
    case "updateAvailable":
      return "Continue";
    default:
      return "Download";
  }
});

const siteDisabled = computed(() => site.value.phase === "finishing");

function startSite() {
  switch (site.value.phase) {
    case "notDownloaded":
      void assistant.download("node");
      break;
    case "downloading":
      void assistant.cancel("node");
      break;
    case "downloadError":
      void assistant.retry("node");
      break;
    case "ready":
    case "updateAvailable":
      step.value = 3;
      break;
    default:
      break;
  }
}

// Step 3: GitHub.
const githubSignedIn = computed(
  () => gh.user != null || status.value?.githubSignedIn === true,
);

// A finished in-app sign-in changes what the daemon reports.
watch(
  () => gh.user,
  (user) => {
    if (user) void recheck();
  },
);

onMounted(() => {
  if (!status.value) {
    void recheck();
  } else if (!assistant.snapshot) {
    void assistant.load().catch(() => {
      // The step falls back to an empty assistant list; Skip setup stays available.
    });
  }
  gh.load().catch(() => {
    // Without GitHub status the account row keeps the manual sign-in hint.
  });
});

const nextLabel = computed(() => {
  if (step.value === 3) return "Start building";
  if (step.value === 1 && selectedRow.value && !selectedRow.value.chatReady) {
    if (selectedRow.value.phase === "downloading") return "Downloading…";
    if (selectedRow.value.phase === "finishing") return "Setting up…";
  }
  return "Continue";
});

const nextDisabled = computed(() => {
  if (step.value === 1) return !selectedRow.value?.chatReady;
  if (step.value === 2) return !siteReady.value;
  return false;
});

function next() {
  if (step.value === 3) {
    emit("continue");
    return;
  }
  if (nextDisabled.value) return;
  step.value = (step.value + 1) as Step;
}

function back() {
  if (step.value > 1) step.value = (step.value - 1) as Step;
}

function goToStep(n: Step) {
  if (n < step.value) step.value = n;
}

function dotLabel(n: Step): string {
  const label = STEPS.find((s) => s.n === n)?.label ?? "";
  if (n === step.value) return `Step ${n} of 3: ${label}`;
  if (n < step.value) return `Go back to step ${n}: ${label}`;
  return `Step ${n} of 3: ${label}`;
}
</script>

<template>
  <div
    class="bg-background text-foreground flex min-h-screen flex-col p-6 sm:p-8"
    data-testid="setup-check"
  >
    <div class="mx-auto flex w-full max-w-xl flex-1 flex-col">
      <header class="flex justify-end">
        <Button
          variant="ghost"
          size="sm"
          data-testid="onboarding-skip"
          @click="emit('continue')"
        >
          Skip setup
        </Button>
      </header>

      <main class="flex flex-1 flex-col justify-center py-10">
        <p v-if="failed" class="text-destructive text-sm" role="alert" data-testid="setup-error">
          We couldn't check your setup just now. Make sure orso is still running, then select
          Recheck.
        </p>
        <p
          v-else-if="!status"
          class="text-muted-foreground flex items-center gap-2 text-sm"
          data-testid="setup-loading"
        >
          <Spinner class="size-3.5" aria-hidden="true" /> Checking your setup…
        </p>

        <!-- 1. Assistant -->
        <section
          v-else-if="step === 1"
          class="flex flex-col gap-8"
          data-testid="onboarding-step-assistant"
        >
          <div class="flex flex-col gap-2">
            <h1 class="text-3xl font-semibold tracking-tight text-balance sm:text-4xl">
              Set up your assistant.
            </h1>
            <p class="text-muted-foreground max-w-xl text-base text-pretty">
              Download an assistant to make changes to your site. OpenCode is free to start; use
              the account you already have for the others.
            </p>
          </div>

          <div class="flex flex-col gap-4">
            <div
              class="bg-card divide-y overflow-hidden rounded-xl border"
              role="radiogroup"
              aria-label="Assistant"
            >
              <div v-for="row in cards" :key="row.id" class="flex flex-col">
                <button
                  type="button"
                  role="radio"
                  :aria-checked="selectedRow?.id === row.id"
                  :data-testid="`onboarding-provider-${row.id}`"
                  :data-selected="selectedRow?.id === row.id ? 'true' : undefined"
                  :data-phase="row.phase"
                  class="flex w-full items-center gap-3 px-4 py-3.5 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/60 focus-visible:ring-inset"
                  :class="
                    selectedRow?.id === row.id
                      ? 'bg-accent/60 ring-2 ring-ring/60 ring-inset'
                      : 'hover:bg-muted/50'
                  "
                  @click="select(row.id)"
                >
                  <span
                    class="bg-background flex size-9 shrink-0 items-center justify-center rounded-lg border"
                  >
                    <ProviderIcon :provider="row.id" class="size-5" />
                  </span>

                  <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                    <span class="flex flex-wrap items-center gap-2">
                      <span class="font-medium">{{ row.name }}</span>
                      <span
                        v-if="row.recommended"
                        class="bg-secondary text-secondary-foreground rounded-full px-2 py-0.5 text-[11px] font-medium"
                      >
                        Recommended
                      </span>
                    </span>
                    <span class="text-muted-foreground text-sm">{{ row.description }}</span>

                    <span class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
                      <span
                        v-if="row.status && row.phase !== 'downloading' && row.phase !== 'finishing'"
                        class="inline-flex items-center gap-1"
                        :class="
                          row.tone === 'destructive'
                            ? 'text-destructive'
                            : row.tone === 'warning'
                              ? 'text-amber-700 dark:text-amber-400'
                              : row.tone === 'success'
                                ? 'text-emerald-700 dark:text-emerald-400'
                                : 'text-muted-foreground'
                        "
                        :data-testid="`onboarding-status-${row.id}`"
                      >
                        <CircleCheck v-if="row.tone === 'success'" class="size-3.5" aria-hidden="true" />
                        <CircleAlert v-else-if="row.tone === 'destructive'" class="size-3.5" aria-hidden="true" />
                        {{ row.status }}
                      </span>
                      <span
                        v-else-if="row.phase === 'downloading'"
                        class="text-muted-foreground tabular-nums"
                        :data-testid="`onboarding-percent-${row.id}`"
                      >
                        {{ row.progress ?? 0 }}%
                      </span>
                      <span
                        v-else-if="row.phase === 'finishing'"
                        class="text-muted-foreground inline-flex items-center gap-1"
                      >
                        <Spinner class="size-3" aria-hidden="true" /> Setting up…
                      </span>
                      <span
                        v-if="
                          (row.phase === 'ready' || row.phase === 'updateAvailable') && row.version
                        "
                        class="text-muted-foreground"
                        :data-testid="`onboarding-version-${row.id}`"
                      >
                        Version {{ row.version }}<template v-if="sourceLabel(row.source)">
                          · {{ sourceLabel(row.source) }}</template
                        >
                      </span>
                      <span
                        v-else-if="row.phase === 'notDownloaded'"
                        class="text-muted-foreground"
                        :data-testid="`onboarding-size-${row.id}`"
                      >
                        {{ formatDownloadSize(row.provider.runtime?.downloadBytes) ?? "" }}
                      </span>
                    </span>
                  </span>

                  <Check
                    v-if="selectedRow?.id === row.id"
                    class="size-5 shrink-0"
                    aria-hidden="true"
                    data-testid="onboarding-selected-check"
                  />
                </button>

                <div
                  v-if="selectedRow?.id === row.id && row.phase === 'downloading'"
                  class="flex items-center gap-3 px-4 pb-4 sm:pl-16"
                  :data-testid="`onboarding-progress-${row.id}`"
                >
                  <Progress
                    :model-value="row.progress ?? 0"
                    :aria-label="`Downloading ${row.name}: ${row.progress ?? 0}%`"
                    class="max-w-64"
                  />
                  <span class="text-muted-foreground text-xs tabular-nums">
                    {{ row.progress ?? 0 }}%
                  </span>
                  <Button
                    variant="outline"
                    size="xs"
                    :data-testid="`onboarding-cancel-${row.id}`"
                    @click="cancelSelected"
                  >
                    <X aria-hidden="true" /> Cancel
                  </Button>
                </div>

                <div
                  v-if="selectedRow?.id === row.id && row.phase === 'downloadError'"
                  class="px-4 pb-4 text-sm sm:pl-16"
                  :data-testid="`onboarding-error-${row.id}`"
                >
                  <p class="text-destructive">
                    We couldn't download {{ row.name }}. Check your connection and try again.
                  </p>
                  <details v-if="row.message" class="mt-1">
                    <summary class="text-muted-foreground cursor-pointer text-xs underline">
                      Details
                    </summary>
                    <p class="text-muted-foreground mt-1 text-xs break-words">{{ row.message }}</p>
                  </details>
                </div>

                <div
                  v-if="selectedRow?.id === row.id && signInState"
                  class="px-4 pb-4 sm:pl-16"
                >
                  <SignInPanel
                    :provider-name="row.name"
                    :state="signInState ?? NO_SIGN_IN"
                    @submit-code="onSubmitCode(row.id, $event)"
                    @cancel="onCancelSignIn(row.id)"
                    @retry="onRetrySignIn(row.id)"
                    @dismiss="onDismissSignIn(row.id)"
                  />
                </div>
              </div>
            </div>

            <div class="flex flex-wrap items-center gap-3">
              <Button
                v-if="selectedRow"
                :disabled="setupDisabled"
                :data-testid="`onboarding-setup-${selectedRow.id}`"
                @click="startSelected"
              >
                <Download
                  v-if="!selectedRow.chatReady && selectedRow.phase === 'notDownloaded'"
                  aria-hidden="true"
                />
                <RefreshCw
                  v-else-if="!selectedRow.chatReady && selectedRow.phase === 'downloadError'"
                  aria-hidden="true"
                />
                <LogIn
                  v-else-if="!selectedRow.chatReady && selectedRow.phase === 'ready'"
                  aria-hidden="true"
                />
                {{ setupLabel }}
              </Button>
            </div>
          </div>

          <section class="flex flex-col gap-3 border-t pt-6" data-testid="onboarding-existing">
            <div class="flex flex-wrap items-start justify-between gap-2">
              <div class="flex flex-col gap-1">
                <h2 class="text-muted-foreground text-xs font-medium tracking-wide uppercase">
                  Or use an assistant you already have.
                </h2>
                <p v-if="existingRows.length" class="text-muted-foreground text-sm">
                  Select one to continue with it.
                </p>
              </div>
              <Button
                variant="ghost"
                size="sm"
                :disabled="checking"
                data-testid="setup-recheck"
                @click="recheck"
              >
                <RefreshCw
                  :class="{ 'animate-spin motion-reduce:animate-none': checking }"
                  aria-hidden="true"
                />
                Recheck
              </Button>
            </div>

            <div v-if="existingRows.length" class="flex flex-col gap-2" role="radiogroup" aria-label="Existing assistants">
              <button
                v-for="row in existingRows"
                :key="row.id"
                type="button"
                role="radio"
                :aria-checked="selectedRow?.id === row.id"
                :data-testid="`onboarding-existing-${row.id}`"
                class="bg-card flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/60 focus-visible:ring-inset"
                :class="
                  selectedRow?.id === row.id
                    ? 'bg-accent/60 ring-2 ring-ring/60 ring-inset'
                    : 'hover:bg-muted/50'
                "
                @click="select(row.id)"
              >
                <span class="bg-background flex size-8 shrink-0 items-center justify-center rounded-md border">
                  <ProviderIcon :provider="row.id" class="size-4" />
                </span>
                <span class="min-w-0 flex-1">
                  <span class="block text-sm font-medium">{{ row.name }}</span>
                  <span class="text-muted-foreground block text-xs">Already on your Mac</span>
                </span>
                <Check v-if="selectedRow?.id === row.id" class="size-4" aria-hidden="true" />
              </button>
            </div>
            <p v-else class="text-muted-foreground text-sm">
              If an assistant is already on your Mac, it will appear here. Select Recheck after
              installing it.
            </p>
          </section>
        </section>

        <!-- 2. Site builder -->
        <section
          v-else-if="step === 2"
          class="flex flex-col gap-8"
          data-testid="onboarding-step-site"
        >
          <div class="flex flex-col gap-2">
            <h1 class="text-3xl font-semibold tracking-tight text-balance sm:text-4xl">
              Add the site builder.
            </h1>
            <p class="text-muted-foreground max-w-xl text-base text-pretty">
              orso uses this to install packages and preview your site. It is downloaded once.
            </p>
          </div>

          <div class="bg-card rounded-xl border p-4 sm:p-5" data-testid="onboarding-site">
            <div class="flex items-start gap-3">
              <span
                class="bg-background flex size-9 shrink-0 items-center justify-center rounded-lg border"
              >
                <Package class="size-5" aria-hidden="true" />
              </span>

              <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                <div class="flex flex-wrap items-center gap-2">
                  <span class="font-medium">Site builder</span>
                  <span
                    v-if="site.phase === 'ready' || site.phase === 'updateAvailable'"
                    class="text-xs font-medium text-emerald-700 dark:text-emerald-400"
                  >
                    Ready
                  </span>
                </div>
                <p class="text-muted-foreground text-sm">Lets orso build and preview your site.</p>
                <p
                  v-if="formatDownloadSize(site.provider.runtime?.downloadBytes)"
                  class="text-muted-foreground text-xs"
                  data-testid="onboarding-site-size"
                >
                  {{ formatDownloadSize(site.provider.runtime?.downloadBytes) }}
                </p>
                <p
                  v-if="
                    (site.phase === 'ready' || site.phase === 'updateAvailable') && site.version
                  "
                  class="text-muted-foreground text-xs"
                  data-testid="onboarding-site-version"
                >
                  Version {{ site.version }}<template v-if="sourceLabel(site.source)">
                    · {{ sourceLabel(site.source) }}</template
                  >
                </p>
              </div>

              <Button
                v-if="sitePresent"
                :variant="
                  site.phase === 'downloading' || site.phase === 'downloadError'
                    ? 'outline'
                    : 'default'
                "
                :disabled="siteDisabled"
                data-testid="onboarding-site-primary"
                @click="startSite"
              >
                <Download
                  v-if="site.phase === 'notDownloaded' || site.phase === 'updateAvailable'"
                  aria-hidden="true"
                />
                <X v-else-if="site.phase === 'downloading'" aria-hidden="true" />
                <RotateCcw v-else-if="site.phase === 'downloadError'" aria-hidden="true" />
                {{ siteLabel }}
              </Button>
            </div>

            <div
              v-if="sitePresent && site.phase === 'downloading'"
              class="mt-3 flex items-center gap-3"
              data-testid="onboarding-site-progress"
            >
              <Progress
                :model-value="site.progress ?? 0"
                :aria-label="`Downloading the site builder: ${site.progress ?? 0}%`"
                class="max-w-72"
              />
              <span class="text-muted-foreground text-xs tabular-nums">
                {{ site.progress ?? 0 }}%
              </span>
            </div>

            <p
              v-if="sitePresent && site.phase === 'downloadError'"
              class="text-destructive mt-3 text-sm"
              data-testid="onboarding-site-error"
            >
              We couldn't download the site builder. Check your connection and try again.
            </p>

            <p v-if="!sitePresent" class="text-muted-foreground mt-3 text-sm">
              This version of orso doesn't need a separate download.
            </p>
          </div>
        </section>

        <!-- 3. GitHub (optional) -->
        <section v-else class="flex flex-col gap-8" data-testid="onboarding-step-github">
          <div class="flex flex-col gap-2">
            <h1 class="text-3xl font-semibold tracking-tight text-balance sm:text-4xl">
              Connect GitHub.
            </h1>
            <p class="text-muted-foreground max-w-xl text-base text-pretty">
              Share previews and publish when you are ready. You can do this later.
            </p>
          </div>

          <ul class="bg-card divide-y overflow-hidden rounded-xl border">
            <li class="flex items-start gap-3 px-4 py-3.5" data-testid="onboarding-git">
              <CircleCheck
                v-if="status.git"
                class="mt-0.5 size-4 shrink-0 text-emerald-600 dark:text-emerald-400"
                aria-hidden="true"
              />
              <CircleAlert v-else class="text-muted-foreground mt-0.5 size-4 shrink-0" aria-hidden="true" />
              <div class="flex min-w-0 flex-1 flex-col gap-1 text-sm">
                <span class="font-medium">Git</span>
                <span class="text-muted-foreground">
                  Keeps the history of every change to your sites.
                </span>
                <a
                  v-if="!status.git"
                  class="inline-flex items-center gap-1 self-start underline"
                  href="https://git-scm.com/downloads/mac"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  Get Git <ExternalLink class="size-3" aria-hidden="true" />
                </a>
              </div>
              <span class="text-muted-foreground text-xs">{{ status.git ? "Ready" : "Optional" }}</span>
            </li>

            <li class="flex items-start gap-3 px-4 py-3.5" data-testid="onboarding-github-tools">
              <CircleCheck
                v-if="status.gh"
                class="mt-0.5 size-4 shrink-0 text-emerald-600 dark:text-emerald-400"
                aria-hidden="true"
              />
              <CircleAlert v-else class="text-muted-foreground mt-0.5 size-4 shrink-0" aria-hidden="true" />
              <div class="flex min-w-0 flex-1 flex-col gap-1 text-sm">
                <span class="font-medium">GitHub tools</span>
                <span class="text-muted-foreground">Connects your sites to GitHub.</span>
                <a
                  v-if="!status.gh"
                  class="inline-flex items-center gap-1 self-start underline"
                  href="https://cli.github.com/"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  Get the GitHub tools <ExternalLink class="size-3" aria-hidden="true" />
                </a>
              </div>
              <span class="text-muted-foreground text-xs">{{ status.gh ? "Ready" : "Optional" }}</span>
            </li>

            <li class="flex items-start gap-3 px-4 py-3.5" data-testid="onboarding-github-account">
              <CircleCheck
                v-if="githubSignedIn"
                class="mt-0.5 size-4 shrink-0 text-emerald-600 dark:text-emerald-400"
                aria-hidden="true"
              />
              <CircleAlert v-else class="text-muted-foreground mt-0.5 size-4 shrink-0" aria-hidden="true" />
              <div class="flex min-w-0 flex-1 flex-col gap-1 text-sm">
                <span class="font-medium">GitHub account</span>
                <span v-if="githubSignedIn" class="text-muted-foreground">
                  <template v-if="gh.user">Signed in as {{ gh.user.login }}. </template>
                  <template v-else>You're signed in to GitHub. </template>
                  Share previews and publish when you are ready.
                </span>
                <template v-else>
                  <span class="text-muted-foreground">
                    Sign in so orso can open your sites and share your changes.
                  </span>
                  <Button
                    v-if="status.gh && gh.deviceFlow"
                    size="sm"
                    variant="outline"
                    class="self-start"
                    data-testid="setup-sign-in"
                    @click="gh.signIn()"
                  >
                    <LogIn aria-hidden="true" /> Sign in with GitHub
                  </Button>
                  <span v-else-if="status.gh" class="text-muted-foreground text-xs">
                    Sign in with GitHub on this Mac, then select Recheck.
                  </span>
                  <span v-else class="text-muted-foreground text-xs">
                    Install the GitHub tools first, then sign in.
                  </span>
                </template>
                <p v-if="gh.error" class="text-destructive text-xs" role="alert">
                  GitHub sign-in didn't finish. Please try again.
                </p>
              </div>
            </li>
          </ul>

          <div class="flex flex-wrap items-center justify-between gap-2">
            <Button
              variant="ghost"
              data-testid="onboarding-github-skip"
              @click="emit('continue')"
            >
              Skip for now
            </Button>
            <Button
              variant="ghost"
              size="sm"
              :disabled="checking"
              data-testid="setup-recheck"
              @click="recheck"
            >
              <RefreshCw
                :class="{ 'animate-spin motion-reduce:animate-none': checking }"
                aria-hidden="true"
              />
              Recheck
            </Button>
          </div>
        </section>
      </main>

      <footer class="grid grid-cols-[1fr_auto_1fr] items-center gap-4 border-t pt-5">
        <div class="flex items-center">
          <PawMark class="text-muted-foreground size-5" />
        </div>

        <div class="flex items-center justify-center gap-2" aria-label="Setup steps">
          <button
            v-for="item in STEPS"
            :key="item.n"
            type="button"
            class="h-2.5 rounded-full transition-colors disabled:cursor-default motion-reduce:transition-none"
            :class="[
              item.n === step ? 'bg-primary w-5' : 'w-2.5',
              item.n < step ? 'bg-muted-foreground/60 hover:bg-foreground' : '',
              item.n > step ? 'bg-muted-foreground/25' : '',
            ]"
            :aria-label="dotLabel(item.n)"
            :aria-current="item.n === step ? 'step' : undefined"
            :disabled="item.n > step"
            :data-testid="`onboarding-dot-${item.n}`"
            @click="goToStep(item.n)"
          />
        </div>

        <div class="flex items-center justify-end gap-2">
          <Button
            variant="ghost"
            :disabled="step === 1"
            data-testid="onboarding-back"
            @click="back"
          >
            Back
          </Button>
          <Button :disabled="nextDisabled" data-testid="onboarding-next" @click="next">
            {{ nextLabel }}
          </Button>
        </div>
      </footer>
    </div>

    <DeviceCodeDialog />
  </div>
</template>
