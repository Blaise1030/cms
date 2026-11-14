<script setup lang="ts">
// App settings → Assistant. The same rows and sign-in steps as the setup dialog,
// without a chat to connect to: a ready row simply stays ready.
import { computed, onMounted, ref } from "vue";
import { BotMessageSquare, CircleAlert, RefreshCw } from "@lucide/vue";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Spinner } from "@/components/ui/spinner";
import type { ProviderSignInState } from "@/core/types";
import ProviderRow from "@/modules/assistant/ProviderRow.vue";
import SignInPanel from "@/modules/assistant/SignInPanel.vue";
import { useAssistantStore } from "@/modules/assistant/store";
import type { AssistantRow, RowAction } from "@/modules/assistant/rows";
import SettingsSection from "./SettingsSection.vue";

const store = useAssistantStore();

const rechecking = ref(false);
const loading = computed(() => !store.snapshot && !store.error);
const rows = computed<AssistantRow[]>(() => store.rows);

onMounted(() => {
  if (!store.snapshot) void store.load();
});

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

/** Never rendered: the `v-if` above the panel guards it, this only keeps the prop non-null. */
const NO_SIGN_IN: ProviderSignInState = { state: "cancelled" };

function onAction(row: AssistantRow, action: RowAction) {
  switch (action) {
    case "download":
    case "update":
      // An update uses the same download route until updates get their own.
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
      // There is no chat to open from settings; the row is already ready.
      break;
    case "none":
      break;
  }
}
</script>

<template>
  <SettingsSection label="Assistants" :icon="BotMessageSquare" data-testid="assistant-settings">
    <template #actions>
      <Button
        variant="outline"
        size="sm"
        :disabled="rechecking"
        data-testid="assistant-settings-recheck"
        @click="recheck"
      >
        <RefreshCw :class="{ 'animate-spin': rechecking }" aria-hidden="true" /> Recheck
      </Button>
    </template>

    <Card class="gap-0 overflow-hidden border-border/50 p-0 shadow-xs">
      <p
        v-if="loading"
        class="text-muted-foreground flex items-center gap-2 p-4 text-sm"
        data-testid="assistant-settings-loading"
      >
        <Spinner class="size-3.5" aria-hidden="true" /> Checking your setup…
      </p>

      <Alert
        v-else-if="store.error && !store.snapshot"
        variant="destructive"
        class="m-4"
        data-testid="assistant-settings-error"
      >
        <CircleAlert aria-hidden="true" />
        <AlertDescription class="flex flex-col items-start gap-2">
          <p>We couldn't check your setup just now. Make sure orso is still running, then check again.</p>
          <Button
            variant="outline"
            size="sm"
            :disabled="rechecking"
            data-testid="assistant-settings-retry"
            @click="recheck"
          >
            <RefreshCw :class="{ 'animate-spin': rechecking }" aria-hidden="true" /> Check again
          </Button>
        </AlertDescription>
      </Alert>

      <Empty v-else-if="!rows.length" class="border-0" data-testid="assistant-settings-empty">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <BotMessageSquare aria-hidden="true" />
          </EmptyMedia>
          <EmptyTitle>No assistants found</EmptyTitle>
          <EmptyDescription>
            Download an assistant to make changes to your site, then check again.
          </EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button variant="outline" size="sm" :disabled="rechecking" @click="recheck">
            <RefreshCw :class="{ 'animate-spin': rechecking }" aria-hidden="true" /> Check again
          </Button>
        </EmptyContent>
      </Empty>

      <ul v-else class="divide-y divide-border [&>li]:px-4" data-testid="assistant-settings-rows">
        <template v-for="row in rows" :key="row.id">
          <ProviderRow :row="row" hide-connected-action @action="onAction(row, $event)" />
          <li v-if="panelFor(row.id)" class="pb-3">
            <SignInPanel
              :provider-name="row.name"
              :state="panelFor(row.id) ?? NO_SIGN_IN"
              @submit-code="store.submitCode(row.id, $event)"
              @cancel="store.cancelSignIn(row.id)"
              @retry="store.signIn(row.id)"
              @dismiss="store.clearSignIn(row.id)"
            />
          </li>
        </template>
      </ul>
    </Card>
  </SettingsSection>
</template>
