<script setup lang="ts">
// First-run setup, in three short steps: pick an assistant, add the site
// builder, then optionally connect GitHub. The wizard keeps the component name
// (and the `continue` event) used by App.vue, but replaces the old single card.
//
// Copy rules: never show "agent", "CLI", "ACP", "SDK", "npx", "PATH" or
// "runtime" to the user. Keep the product words: assistant, site, download.
import { computed, onMounted, ref, watch } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UAvatar from "@nuxt/ui/components/Avatar.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UCard from "@nuxt/ui/components/Card.vue";
import UCollapsible from "@nuxt/ui/components/Collapsible.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UPageHeader from "@nuxt/ui/components/PageHeader.vue";
import URadioGroup from "@nuxt/ui/components/RadioGroup.vue";
import UProgress from "@nuxt/ui/components/Progress.vue";
import USeparator from "@nuxt/ui/components/Separator.vue";
import { useDaemon } from "@/core/daemon";
import type { ProviderSignInState, RuntimeStatus, SetupProvider, SetupStatus } from "@/core/types";
import PawMark from "@/app/PawMark.vue";
import ProviderIcon from "@/modules/chat/ProviderIcon.vue";
import {
  PROVIDER_NAMES,
  buildRows,
  deriveRow,
  type AssistantRow,
  type RowTone,
} from "@/modules/assistant/rows";
import { formatDownloadSize, sourceLabel } from "@/modules/assistant/presentation";
import SignInPanel from "@/modules/assistant/SignInPanel.vue";
import { useAssistantStore } from "@/modules/assistant/store";
import DeviceCodeDialog from "@/modules/github/DeviceCodeDialog.vue";
import SettingsRow from "@/modules/settings/SettingsRow.vue";
import { useGithubStore } from "@/modules/github/store";
import { onboardingStartStep } from "@/app/setup";

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

/** Step titles: a page header without its default rule and padding. */
const HEADER_UI = {
  root: "border-none py-0",
  title: "font-semibold tracking-tight text-balance",
  description: "mt-2 max-w-xl text-base",
};

/** The square tile behind a provider or package icon. */
const AVATAR_UI = { root: "bg-background rounded-lg border border-default", icon: "text-default size-5" };

function toneColor(tone: RowTone): "success" | "warning" | "error" | "neutral" {
  if (tone === "success") return "success";
  if (tone === "warning") return "warning";
  if (tone === "destructive") return "error";
  return "neutral";
}

function toneIcon(tone: RowTone): string | undefined {
  if (tone === "success") return "i-hugeicons-checkmark-circle-02";
  if (tone === "destructive") return "i-hugeicons-alert-circle";
  return undefined;
}

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
/** Radio items for the two assistant groups; `row` feeds the custom label and description slots. */
const cardItems = computed(() => cards.value.map((row) => ({ value: row.id, row })));
const existingItems = computed(() =>
  existingRows.value.map((row) => ({ value: row.id, row, description: "Already on your Mac" })),
);
/** Each group shows the selection only when the selected assistant is one of its own. */
const selectedCardId = computed(() =>
  cards.value.some((r) => r.id === selectedRow.value?.id) ? selectedRow.value?.id : undefined,
);
const selectedExistingId = computed(() =>
  existingRows.value.some((r) => r.id === selectedRow.value?.id) ? selectedRow.value?.id : undefined,
);

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

const setupIcon = computed(() => {
  const row = selectedRow.value;
  if (!row || row.chatReady) return undefined;
  if (row.phase === "notDownloaded") return "i-hugeicons-download-01";
  if (row.phase === "downloadError") return "i-hugeicons-refresh";
  if (row.phase === "ready") return "i-hugeicons-login-03";
  return undefined;
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

/** Cancel and retry are the secondary look; download and continue are primary. */
const siteSecondary = computed(
  () => site.value.phase === "downloading" || site.value.phase === "downloadError",
);

const siteIcon = computed(() => {
  switch (site.value.phase) {
    case "notDownloaded":
    case "updateAvailable":
      return "i-hugeicons-download-01";
    case "downloading":
      return "i-hugeicons-cancel-01";
    case "downloadError":
      return "i-hugeicons-arrow-turn-backward";
    default:
      return undefined;
  }
});

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

/** Git and the GitHub tools share one row shape: ready, or a link to get it. */
const tools = computed(() => [
  {
    id: "git",
    testId: "onboarding-git",
    icon: "i-hugeicons-git-branch",
    title: "Git",
    description: "Keeps the history of every change to your sites.",
    ready: status.value?.git === true,
    getLabel: "Get Git",
    getUrl: "https://git-scm.com/downloads/mac",
  },
  {
    id: "tools",
    testId: "onboarding-github-tools",
    icon: "i-hugeicons-git-pull-request",
    title: "GitHub tools",
    description: "Connects your sites to GitHub.",
    ready: status.value?.gh === true,
    getLabel: "Get the GitHub tools",
    getUrl: "https://cli.github.com/",
  },
]);

// A finished in-app sign-in changes what the daemon reports.
watch(
  () => gh.user,
  (user) => {
    if (user) void recheck();
  },
);

/** Prefer the live assistant snapshot; fall back to the status the shell already fetched. */
function resolveSiteReady(): boolean {
  const node = assistant.node ?? status.value?.node ?? null;
  if (node == null) {
    // No status yet: stay put. Status without node: older daemon, site builder not required.
    return status.value != null;
  }
  return node.phase === "ready" || node.phase === "updateAvailable";
}

/** Skip finished steps so a Connected assistant does not reopen on step 1. */
function syncStartStep() {
  const chatReady =
    cards.value.some((row) => row.chatReady) ||
    status.value?.providers.some((provider) => provider.chatReady) === true;
  const target = onboardingStartStep({ chatReady, siteReady: resolveSiteReady() });
  if (target > step.value) step.value = target;
  if (chatReady) {
    const ready =
      cards.value.find((row) => row.chatReady) ??
      status.value?.providers.find((provider) => provider.chatReady);
    if (ready) selectedId.value = ready.id;
  }
}

watch([() => assistant.providers, () => assistant.node, status], syncStartStep, { deep: true });

onMounted(() => {
  if (!status.value) {
    void recheck().finally(syncStartStep);
  } else if (!assistant.snapshot) {
    void assistant
      .load()
      .catch(() => {
        // The step falls back to an empty assistant list; Skip setup stays available.
      })
      .finally(syncStartStep);
  } else {
    syncStartStep();
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
    class="bg-background text-default flex min-h-screen flex-col p-6 sm:p-8"
    data-testid="setup-check"
  >
    <div class="mx-auto flex w-full max-w-xl flex-1 flex-col">
      <header class="flex justify-end">
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          label="Skip setup"
          data-testid="onboarding-skip"
          @click="emit('continue')"
        />
      </header>

      <main class="flex flex-1 flex-col justify-center py-10">
        <UAlert
          v-if="failed"
          color="error"
          variant="subtle"
          icon="i-hugeicons-alert-circle"
          role="alert"
          description="We couldn't check your setup just now. Make sure orso is still running, then select Recheck."
          data-testid="setup-error"
        />
        <p
          v-else-if="!status"
          class="text-muted flex items-center gap-2 text-sm"
          data-testid="setup-loading"
        >
          <UIcon name="i-hugeicons-loading-03" class="size-3.5 animate-spin" aria-hidden="true" />
          Checking your setup…
        </p>

        <!-- 1. Assistant -->
        <section
          v-else-if="step === 1"
          class="flex flex-col gap-8"
          data-testid="onboarding-step-assistant"
        >
          <UPageHeader
            title="Set up your assistant."
            description="Download an assistant to make changes to your site. OpenCode is free to start; use the account you already have for the others."
            :ui="HEADER_UI"
          />

          <div class="flex flex-col gap-4">
            <URadioGroup
              :model-value="selectedCardId"
              :items="cardItems"
              variant="card"
              indicator="start"
              aria-label="Assistant"
              class="w-full"
              :ui="{ fieldset: 'gap-2', item: 'w-full items-start', wrapper: 'min-w-0 flex-1' }"
              @update:model-value="(id) => select(String(id))"
            >
              <template #label="{ item }">
                <span
                  class="flex flex-wrap items-center gap-2"
                  :data-testid="`onboarding-provider-${item.row.id}`"
                  :data-selected="selectedRow?.id === item.row.id ? 'true' : undefined"
                  :data-phase="item.row.phase"
                >
                  <ProviderIcon :provider="item.row.id" class="size-4" />
                  <span class="text-highlighted font-medium">{{ item.row.name }}</span>
                  <UBadge
                    v-if="item.row.recommended"
                    color="neutral"
                    variant="subtle"
                    size="sm"
                    class="rounded-full"
                    label="Recommended"
                  />
                </span>
              </template>
              <template #description="{ item }">
                <span class="flex flex-col gap-1">
                  <span>{{ item.row.description }}</span>
                  <span class="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
                    <UBadge
                      v-if="item.row.status && item.row.phase !== 'downloading' && item.row.phase !== 'finishing'"
                      :color="toneColor(item.row.tone)"
                      variant="soft"
                      size="sm"
                      :icon="toneIcon(item.row.tone)"
                      :label="item.row.status"
                      :data-testid="`onboarding-status-${item.row.id}`"
                    />
                    <span
                      v-else-if="item.row.phase === 'downloading'"
                      class="tabular-nums"
                      :data-testid="`onboarding-percent-${item.row.id}`"
                    >
                      {{ item.row.progress ?? 0 }}%
                    </span>
                    <span v-else-if="item.row.phase === 'finishing'" class="inline-flex items-center gap-1">
                      <UIcon name="i-hugeicons-loading-03" class="size-3 animate-spin" aria-hidden="true" />
                      Setting up…
                    </span>
                    <span
                      v-if="(item.row.phase === 'ready' || item.row.phase === 'updateAvailable') && item.row.version"
                      :data-testid="`onboarding-version-${item.row.id}`"
                    >
                      Version {{ item.row.version }}<template v-if="sourceLabel(item.row.source)">
                        · {{ sourceLabel(item.row.source) }}</template
                      >
                    </span>
                    <span
                      v-else-if="item.row.phase === 'notDownloaded'"
                      :data-testid="`onboarding-size-${item.row.id}`"
                    >
                      {{ formatDownloadSize(item.row.provider.runtime?.downloadBytes) ?? "" }}
                    </span>
                  </span>
                </span>
              </template>
            </URadioGroup>

            <!-- The selected assistant's controls sit under the group, outside the option labels. -->
            <template v-if="selectedRow">
              <div
                v-if="selectedRow.phase === 'downloading'"
                class="flex items-center gap-3"
                :data-testid="`onboarding-progress-${selectedRow.id}`"
              >
                <UProgress
                  :model-value="selectedRow.progress ?? 0"
                  size="sm"
                  :get-value-label="() => `Downloading ${selectedRow?.name}: ${selectedRow?.progress ?? 0}%`"
                  class="max-w-64"
                />
                <span class="text-muted text-xs tabular-nums">{{ selectedRow.progress ?? 0 }}%</span>
                <UButton
                  color="neutral"
                  variant="outline"
                  size="xs"
                  icon="i-hugeicons-cancel-01"
                  label="Cancel"
                  :data-testid="`onboarding-cancel-${selectedRow.id}`"
                  @click="cancelSelected"
                />
              </div>

              <UAlert
                v-if="selectedRow.phase === 'downloadError'"
                color="error"
                variant="subtle"
                icon="i-hugeicons-alert-circle"
                :data-testid="`onboarding-error-${selectedRow.id}`"
              >
                <template #description>
                  <p>We couldn't download {{ selectedRow.name }}. Check your connection and try again.</p>
                  <UCollapsible v-if="selectedRow.message" :unmount-on-hide="false" class="mt-1">
                    <UButton
                      variant="link"
                      color="neutral"
                      size="xs"
                      class="px-0 underline"
                      label="Details"
                      data-testid="onboarding-error-details-toggle"
                    />
                    <template #content>
                      <p class="mt-1 text-xs break-words">{{ selectedRow.message }}</p>
                    </template>
                  </UCollapsible>
                </template>
              </UAlert>

              <SignInPanel
                v-if="signInState"
                :provider-name="selectedRow.name"
                :state="signInState ?? NO_SIGN_IN"
                @submit-code="onSubmitCode(selectedRow.id, $event)"
                @cancel="onCancelSignIn(selectedRow.id)"
                @retry="onRetrySignIn(selectedRow.id)"
                @dismiss="onDismissSignIn(selectedRow.id)"
              />
            </template>

            <div class="flex flex-wrap items-center gap-3">
              <UButton
                v-if="selectedRow"
                :icon="setupIcon"
                :label="setupLabel"
                :disabled="setupDisabled"
                :data-testid="`onboarding-setup-${selectedRow.id}`"
                @click="startSelected"
              />
            </div>
          </div>

          <USeparator />

          <section class="flex flex-col gap-3" data-testid="onboarding-existing">
            <div class="flex flex-wrap items-start justify-between gap-2">
              <div class="flex flex-col gap-1">
                <h2 class="text-muted text-xs font-medium tracking-wide uppercase">
                  Or use an assistant you already have.
                </h2>
                <p v-if="existingRows.length" class="text-muted text-sm">
                  Select one to continue with it.
                </p>
              </div>
              <UButton
                color="neutral"
                variant="ghost"
                size="sm"
                icon="i-hugeicons-refresh"
                loading-icon="i-hugeicons-refresh"
                label="Recheck"
                :loading="checking"
                :disabled="checking"
                data-testid="setup-recheck"
                @click="recheck"
              />
            </div>

            <URadioGroup
              v-if="existingRows.length"
              :model-value="selectedExistingId"
              :items="existingItems"
              variant="card"
              indicator="start"
              aria-label="Existing assistants"
              class="w-full"
              :ui="{ fieldset: 'gap-2', item: 'w-full items-start', wrapper: 'min-w-0 flex-1' }"
              @update:model-value="(id) => select(String(id))"
            >
              <template #label="{ item }">
                <span class="flex items-center gap-2" :data-testid="`onboarding-existing-${item.row.id}`">
                  <ProviderIcon :provider="item.row.id" class="size-4" />
                  <span class="text-highlighted font-medium">{{ item.row.name }}</span>
                </span>
              </template>
            </URadioGroup>
            <p v-else class="text-muted text-sm">
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
          <UPageHeader
            title="Add the site builder."
            description="orso uses this to install packages and preview your site. It is downloaded once."
            :ui="HEADER_UI"
          />

          <UCard
            variant="outline"
            :ui="{ root: 'rounded-xl', body: 'flex flex-col gap-3 p-4 sm:p-5' }"
            data-testid="onboarding-site"
          >
            <div class="flex items-start gap-3">
              <UAvatar size="lg" icon="i-hugeicons-package" :ui="AVATAR_UI" />

              <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                <div class="flex flex-wrap items-center gap-2">
                  <span class="text-highlighted font-medium">Site builder</span>
                  <UBadge
                    v-if="site.phase === 'ready' || site.phase === 'updateAvailable'"
                    color="success"
                    variant="subtle"
                    size="sm"
                    label="Ready"
                  />
                </div>
                <p class="text-muted text-sm">Lets orso build and preview your site.</p>
                <p
                  v-if="formatDownloadSize(site.provider.runtime?.downloadBytes)"
                  class="text-muted text-xs"
                  data-testid="onboarding-site-size"
                >
                  {{ formatDownloadSize(site.provider.runtime?.downloadBytes) }}
                </p>
                <p
                  v-if="
                    (site.phase === 'ready' || site.phase === 'updateAvailable') && site.version
                  "
                  class="text-muted text-xs"
                  data-testid="onboarding-site-version"
                >
                  Version {{ site.version }}<template v-if="sourceLabel(site.source)">
                    · {{ sourceLabel(site.source) }}</template
                  >
                </p>
              </div>

              <UButton
                v-if="sitePresent"
                :color="siteSecondary ? 'neutral' : 'primary'"
                :variant="siteSecondary ? 'outline' : 'solid'"
                :icon="siteIcon"
                :label="siteLabel"
                :disabled="siteDisabled"
                data-testid="onboarding-site-primary"
                @click="startSite"
              />
            </div>

            <div
              v-if="sitePresent && site.phase === 'downloading'"
              class="flex items-center gap-3"
              data-testid="onboarding-site-progress"
            >
              <UProgress
                :model-value="site.progress ?? 0"
                size="sm"
                :get-value-label="() => `Downloading the site builder: ${site.progress ?? 0}%`"
                class="max-w-72"
              />
              <span class="text-muted text-xs tabular-nums">
                {{ site.progress ?? 0 }}%
              </span>
            </div>

            <UAlert
              v-if="sitePresent && site.phase === 'downloadError'"
              color="error"
              variant="subtle"
              icon="i-hugeicons-alert-circle"
              description="We couldn't download the site builder. Check your connection and try again."
              data-testid="onboarding-site-error"
            />

            <p v-if="!sitePresent" class="text-muted text-sm">
              This version of orso doesn't need a separate download.
            </p>
          </UCard>
        </section>

        <!-- 3. GitHub (optional) -->
        <section v-else class="flex flex-col gap-8" data-testid="onboarding-step-github">
          <UPageHeader
            title="Connect GitHub."
            description="Share previews and publish when you are ready. You can do this later."
            :ui="HEADER_UI"
          />

          <UCard
            variant="outline"
            :ui="{ root: 'overflow-hidden rounded-xl', body: 'divide-y divide-default p-0 sm:p-0' }"
          >
            <SettingsRow
              v-for="tool in tools"
              :key="tool.id"
              :icon="tool.icon"
              :description="tool.description"
              :data-testid="tool.testId"
            >
              <template #title>
                <UIcon
                  :name="tool.ready ? 'i-hugeicons-checkmark-circle-02' : 'i-hugeicons-alert-circle'"
                  :class="tool.ready ? 'text-success' : 'text-muted'"
                  class="size-3.5"
                  aria-hidden="true"
                />
                {{ tool.title }}
              </template>
              <UButton
                v-if="!tool.ready"
                variant="link"
                color="neutral"
                size="sm"
                class="self-start px-0"
                trailing-icon="i-hugeicons-link-square-02"
                :label="tool.getLabel"
                :to="tool.getUrl"
                target="_blank"
              />
              <template #actions>
                <UBadge
                  color="neutral"
                  :variant="tool.ready ? 'subtle' : 'outline'"
                  :label="tool.ready ? 'Ready' : 'Optional'"
                />
              </template>
            </SettingsRow>

            <SettingsRow
              icon="i-hugeicons-user-circle"
              align="start"
              data-testid="onboarding-github-account"
            >
              <template #title>
                <UIcon
                  :name="githubSignedIn ? 'i-hugeicons-checkmark-circle-02' : 'i-hugeicons-alert-circle'"
                  :class="githubSignedIn ? 'text-success' : 'text-muted'"
                  class="size-3.5"
                  aria-hidden="true"
                />
                GitHub account
              </template>
              <template #description>
                <template v-if="githubSignedIn">
                  <template v-if="gh.user">Signed in as {{ gh.user.login }}. </template>
                  <template v-else>You're signed in to GitHub. </template>
                  Share previews and publish when you are ready.
                </template>
                <template v-else>Sign in so orso can open your sites and share your changes.</template>
              </template>

              <template v-if="!githubSignedIn">
                <UButton
                  v-if="status.gh && gh.deviceFlow"
                  color="neutral"
                  variant="outline"
                  size="sm"
                  class="self-start"
                  icon="i-hugeicons-login-03"
                  label="Sign in with GitHub"
                  data-testid="setup-sign-in"
                  @click="gh.signIn()"
                />
                <p v-else-if="status.gh" class="text-muted text-xs">
                  Sign in with GitHub on this Mac, then select Recheck.
                </p>
                <p v-else class="text-muted text-xs">
                  Install the GitHub tools first, then sign in.
                </p>
              </template>
              <p v-if="gh.error" class="text-error text-xs" role="alert">
                GitHub sign-in didn't finish. Please try again.
              </p>

              <template #actions>
                <UBadge
                  color="neutral"
                  :variant="githubSignedIn ? 'subtle' : 'outline'"
                  :label="githubSignedIn ? 'Ready' : 'Optional'"
                />
              </template>
            </SettingsRow>
          </UCard>

          <div class="flex flex-wrap items-center justify-between gap-2">
            <UButton
              color="neutral"
              variant="ghost"
              label="Skip for now"
              data-testid="onboarding-github-skip"
              @click="emit('continue')"
            />
            <UButton
              color="neutral"
              variant="ghost"
              size="sm"
              icon="i-hugeicons-refresh"
              loading-icon="i-hugeicons-refresh"
              label="Recheck"
              :loading="checking"
              :disabled="checking"
              data-testid="setup-recheck"
              @click="recheck"
            />
          </div>
        </section>
      </main>

      <footer class="grid grid-cols-[1fr_auto_1fr] items-center gap-4 pt-5">
        <div class="flex items-center">
          <PawMark class="text-muted size-5" />
        </div>

        <div class="flex items-center justify-center gap-2" aria-label="Setup steps">
          <UButton
            v-for="item in STEPS"
            :key="item.n"
            :color="item.n === step ? 'primary' : 'neutral'"
            variant="solid"
            class="h-2.5 rounded-full p-0 transition-colors disabled:cursor-default disabled:opacity-100 motion-reduce:transition-none"
            :class="[
              item.n === step ? 'bg-primary w-5' : 'w-2.5',
              item.n < step ? 'bg-inverted/40 hover:bg-inverted' : '',
              item.n > step ? 'bg-inverted/15 disabled:bg-inverted/15' : '',
            ]"
            :aria-label="dotLabel(item.n)"
            :aria-current="item.n === step ? 'step' : undefined"
            :disabled="item.n > step"
            :data-testid="`onboarding-dot-${item.n}`"
            @click="goToStep(item.n)"
          />
        </div>

        <div class="flex items-center justify-end gap-2">
          <UButton
            color="neutral"
            variant="ghost"
            label="Back"
            :disabled="step === 1"
            data-testid="onboarding-back"
            @click="back"
          />
          <UButton
            :label="nextLabel"
            :disabled="nextDisabled"
            data-testid="onboarding-next"
            @click="next"
          />
        </div>
      </footer>
    </div>

    <DeviceCodeDialog />
  </div>
</template>
