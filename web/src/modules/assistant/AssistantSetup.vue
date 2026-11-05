<script setup lang="ts">
// Assistant setup: the list of assistants, the separate site-builder download,
// and the sign-in steps. It works embedded (SetupCheck) and inside a dialog:
// the data comes from the assistant store, `initial` is a read-only fallback
// for previews and tests, and the caller owns Continue / dismissal.
import { computed, onMounted, ref } from "vue";
import { ChevronDown, CircleAlert, RefreshCw } from "@lucide/vue";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
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
      class="text-muted-foreground flex items-center gap-2 text-sm"
      data-testid="assistant-loading"
    >
      <Spinner class="size-3.5" aria-hidden="true" /> Checking your setup…
    </p>

    <Alert v-else-if="error" variant="destructive" data-testid="assistant-load-error">
      <CircleAlert aria-hidden="true" />
      <AlertDescription class="flex flex-col items-start gap-2">
        <p>We couldn't check your setup just now. Make sure orso is still running, then check again.</p>
        <Button
          variant="outline"
          size="sm"
          :disabled="rechecking"
          data-testid="assistant-load-retry"
          @click="recheck"
        >
          <RefreshCw :class="{ 'animate-spin': rechecking }" aria-hidden="true" /> Check again
        </Button>
      </AlertDescription>
    </Alert>

    <template v-else>
      <section class="flex flex-col gap-2" data-testid="assistant-assistants">
        <div class="flex flex-col gap-1">
          <h3 class="text-sm font-medium">Assistants</h3>
          <p class="text-muted-foreground text-xs">
            Pick an assistant to make the changes you ask for. You can add another later.
          </p>
        </div>

        <p v-if="!rows.length" class="text-muted-foreground text-sm">
          No assistants were found. Select Check again below.
        </p>

        <ul v-else class="flex flex-col divide-y" data-testid="assistant-rows">
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

        <details class="group" data-testid="assistant-installed">
          <summary
            class="text-muted-foreground hover:text-foreground inline-flex cursor-pointer list-none items-center gap-1.5 text-sm"
          >
            <ChevronDown
              class="size-4 transition-transform group-open:rotate-180"
              aria-hidden="true"
            />
            I already installed an assistant
          </summary>
          <div class="text-muted-foreground mt-2 flex flex-col items-start gap-2 text-sm">
            <p>orso can use an assistant you installed yourself. Select Check again once it's ready.</p>
            <Button
              variant="outline"
              size="sm"
              :disabled="rechecking"
              data-testid="assistant-recheck"
              @click="recheck"
            >
              <RefreshCw :class="{ 'animate-spin': rechecking }" aria-hidden="true" /> Check again
            </Button>
          </div>
        </details>
      </section>

      <section
        v-if="showSiteBuilder"
        class="flex flex-col gap-2"
        data-testid="assistant-site-builder"
      >
        <div class="flex flex-col gap-1">
          <h3 class="text-sm font-medium">Site builder</h3>
          <p class="text-muted-foreground text-xs">
            orso needs this to create a project and preview your site. You can start chatting
            first.
          </p>
        </div>
        <ul class="flex flex-col divide-y">
          <ProviderRow :row="siteBuilder" @action="onAction(siteBuilder, $event)" />
        </ul>
      </section>
    </template>

    <div v-if="showContinue" class="flex justify-end">
      <Button data-testid="assistant-continue" @click="emit('continue')">Continue</Button>
    </div>
  </div>
</template>
