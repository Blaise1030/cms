<script setup lang="ts">
// Assistant setup: the list of assistants, the separate site-builder download,
// and the sign-in steps. It works embedded (SetupCheck) and inside a dialog:
// the data comes from the assistant store, `initial` is a read-only fallback
// for previews and tests, and the caller owns Continue / dismissal.
import { computed, onMounted, ref } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UCollapsible from "@nuxt/ui/components/Collapsible.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import type { ProviderSignInState } from "@/core/types";
import ProviderRow from "./ProviderRow.vue";
import SignInPanel from "./SignInPanel.vue";
import { buildRows, siteBuilderRow, type AssistantRow, type RowAction } from "./rows";
import { useAssistantStore, type AssistantSnapshot } from "./store";

const props = withDefaults(
  defineProps<{
    /** Read-only data for previews/tests. When set, the component does not self-load. */
    initial?: AssistantSnapshot | null;
    /** Load the snapshot when the store has none. Default true. */
    autoload?: boolean;
    /** Render a caller-dismissed Continue button. Embedded callers leave it off. */
    showContinue?: boolean;
  }>(),
  { initial: null, autoload: true, showContinue: false },
);

const emit = defineEmits<{
  /** The caller should move on / close its dialog. */
  continue: [];
  /** The user picked a ready assistant; the caller opens a chat with it. */
  connect: [id: string];
}>();

const store = useAssistantStore();

const snapshot = computed(() => props.initial ?? store.snapshot);
const ready = computed(() => snapshot.value !== null);
const rows = computed<AssistantRow[]>(() =>
  props.initial ? buildRows(props.initial.providers) : store.rows,
);
const siteBuilder = computed<AssistantRow>(() =>
  props.initial ? siteBuilderRow(props.initial.node) : store.siteBuilder,
);
const showSiteBuilder = computed(() => snapshot.value?.node != null);
const loading = computed(() => !ready.value && !store.error);
const error = computed(() => (ready.value ? null : store.error));

onMounted(() => {
  if (!props.autoload || props.initial || store.snapshot) return;
  void store.load();
});

const rechecking = ref(false);

async function recheck() {
  rechecking.value = true;
  try {
    await store.load();
  } finally {
    rechecking.value = false;
  }
}

/** The sign-in step to show under a row, if one is waiting on the user. */
function panelFor(id: string): ProviderSignInState | null {
  const state = store.signInFor(id);
  if (!state) return null;
  return state.state === "browser" || state.state === "code" || state.state === "failed"
    ? state
    : null;
}

/** Never rendered: `v-if` above the panel guards it, this only keeps the prop non-null. */
const NO_SIGN_IN: ProviderSignInState = { state: "cancelled" };

function onAction(row: AssistantRow, action: RowAction) {
  switch (action) {
    case "download":
      // An update uses the same download route until updates get their own.
    case "update":
      void store.download(row.id);
      break;
    case "retry":
      void store.retry(row.id);
      break;
    case "cancel":
      void store.cancel(row.id);
      break;
    case "signIn":
      void store.signIn(row.id);
      break;
    case "connect":
      emit("connect", row.id);
      break;
    case "none":
      break;
  }
}

function onSubmitCode(id: string, code: string) {
  void store.submitCode(id, code);
}

function onCancelSignIn(id: string) {
  void store.cancelSignIn(id);
}

function onRetrySignIn(id: string) {
  void store.signIn(id);
}

function onDismissSignIn(id: string) {
  store.clearSignIn(id);
}
</script>

<template>
  <div class="flex flex-col gap-5" data-testid="assistant-setup">
    <p
      v-if="loading"
      class="text-muted flex items-center gap-2 text-sm"
      data-testid="assistant-loading"
    >
      <UIcon name="i-hugeicons-loading-03" class="size-3.5 animate-spin" aria-hidden="true" />
      Checking your setup…
    </p>

    <UAlert
      v-else-if="error"
      color="error"
      variant="subtle"
      icon="i-hugeicons-alert-circle"
      description="We couldn't check your setup just now. Make sure orso is still running, then check again."
      data-testid="assistant-load-error"
    >
      <template #actions>
        <UButton
          color="neutral"
          variant="outline"
          size="sm"
          icon="i-hugeicons-refresh"
          loading-icon="i-hugeicons-refresh"
          label="Check again"
          :loading="rechecking"
          :disabled="rechecking"
          data-testid="assistant-load-retry"
          @click="recheck"
        />
      </template>
    </UAlert>

    <template v-else>
      <section class="flex flex-col gap-2" data-testid="assistant-assistants">
        <div class="flex flex-col gap-1">
          <h3 class="text-highlighted text-sm font-medium">Assistants</h3>
          <p class="text-muted text-xs">
            Pick an assistant to make the changes you ask for. You can add another later.
          </p>
        </div>

        <p v-if="!rows.length" class="text-muted text-sm">
          No assistants were found. Select Check again below.
        </p>

        <ul v-else class="flex flex-col divide-y divide-default" data-testid="assistant-rows">
          <template v-for="row in rows" :key="row.id">
            <ProviderRow :row="row" @action="onAction(row, $event)" />
            <li v-if="panelFor(row.id)" class="pb-3">
              <SignInPanel
                :provider-name="row.name"
                :state="panelFor(row.id) ?? NO_SIGN_IN"
                @submit-code="onSubmitCode(row.id, $event)"
                @cancel="onCancelSignIn(row.id)"
                @retry="onRetrySignIn(row.id)"
                @dismiss="onDismissSignIn(row.id)"
              />
            </li>
          </template>
        </ul>

        <UCollapsible :unmount-on-hide="false" data-testid="assistant-installed">
          <template #default="{ open }">
            <UButton
              color="neutral"
              variant="link"
              size="sm"
              class="px-0"
              icon="i-hugeicons-arrow-down-01"
              :ui="{ leadingIcon: ['transition-transform', open ? 'rotate-180' : ''] }"
              label="I already installed an assistant"
              data-testid="assistant-installed-toggle"
            />
          </template>
          <template #content>
            <div class="text-muted mt-2 flex flex-col items-start gap-2 text-sm">
              <p>orso can use an assistant you installed yourself. Select Check again once it's ready.</p>
              <UButton
                color="neutral"
                variant="outline"
                size="sm"
                icon="i-hugeicons-refresh"
                loading-icon="i-hugeicons-refresh"
                label="Check again"
                :loading="rechecking"
                :disabled="rechecking"
                data-testid="assistant-recheck"
                @click="recheck"
              />
            </div>
          </template>
        </UCollapsible>
      </section>

      <section
        v-if="showSiteBuilder"
        class="flex flex-col gap-2"
        data-testid="assistant-site-builder"
      >
        <div class="flex flex-col gap-1">
          <h3 class="text-highlighted text-sm font-medium">Site builder</h3>
          <p class="text-muted text-xs">
            orso needs this to create a project and preview your site. You can start chatting
            first.
          </p>
        </div>
        <ul class="flex flex-col divide-y divide-default">
          <ProviderRow :row="siteBuilder" @action="onAction(siteBuilder, $event)" />
        </ul>
      </section>
    </template>

    <div v-if="showContinue" class="flex justify-end">
      <UButton label="Continue" data-testid="assistant-continue" @click="emit('continue')" />
    </div>
  </div>
</template>
