<script setup lang="ts">
// The sign-in step for one assistant, shown under its row while a login is
// pending. Three shapes: waiting in the browser, asking for a pasted code, and
// "it didn't finish" with a retry. The panel is presentational: the parent owns
// the store calls, so every button is an event.
import { computed, ref } from "vue";
import { CircleAlert, ExternalLink } from "@lucide/vue";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import type { ProviderSignInState } from "@/core/types";

const props = defineProps<{
  providerName: string;
  state: ProviderSignInState;
}>();

const emit = defineEmits<{
  submitCode: [code: string];
  cancel: [];
  retry: [];
  /** Hide the panel without cancelling anything on the daemon. */
  dismiss: [];
}>();

const code = ref("");
const canSubmit = computed(() => code.value.trim().length > 0);

function submit() {
  const value = code.value.trim();
  if (value) emit("submitCode", value);
}
</script>

<template>
  <div
    v-if="state.state === 'browser' || state.state === 'code' || state.state === 'failed'"
    class="bg-card flex flex-col gap-2 rounded-lg border p-3 text-sm"
    role="group"
    :aria-label="`Sign in to ${providerName}`"
    :data-state="state.state"
    data-testid="assistant-sign-in"
  >
    <template v-if="state.state === 'browser'">
      <p class="font-medium">Finish signing in to {{ providerName }}</p>
      <p class="text-muted-foreground">
        Complete the sign-in in the browser window orso opened.
      </p>
      <p v-if="state.message" class="text-muted-foreground">{{ state.message }}</p>
      <div class="flex flex-wrap items-center gap-2">
        <Button
          v-if="state.url"
          as="a"
          :href="state.url"
          target="_blank"
          rel="noopener noreferrer"
          variant="outline"
          size="sm"
          data-testid="assistant-sign-in-open"
        >
          <ExternalLink aria-hidden="true" /> Open the page again
        </Button>
        <Button
          variant="ghost"
          size="sm"
          data-testid="assistant-sign-in-cancel"
          @click="emit('cancel')"
        >
          Cancel
        </Button>
      </div>
      <p class="text-muted-foreground flex items-center gap-1.5 text-xs">
        <Spinner class="size-3.5" aria-hidden="true" /> Waiting for you to finish…
      </p>
    </template>

    <template v-else-if="state.state === 'code'">
      <p class="font-medium">Enter the code to sign in to {{ providerName }}</p>
      <p v-if="state.message" class="text-muted-foreground">{{ state.message }}</p>
      <form class="flex flex-wrap items-center gap-2" @submit.prevent="submit">
        <Input
          v-model="code"
          class="max-w-52"
          aria-label="Sign-in code"
          placeholder="Paste the code"
          autocomplete="one-time-code"
          data-testid="assistant-sign-in-code"
        />
        <Button
          type="submit"
          size="sm"
          :disabled="!canSubmit"
          data-testid="assistant-sign-in-submit"
        >
          Continue
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          data-testid="assistant-sign-in-cancel"
          @click="emit('cancel')"
        >
          Cancel
        </Button>
      </form>
      <a
        v-if="state.url"
        class="inline-flex items-center gap-1 self-start text-xs underline"
        :href="state.url"
        target="_blank"
        rel="noopener noreferrer"
        data-testid="assistant-sign-in-open"
      >
        Open the sign-in page <ExternalLink class="size-3" aria-hidden="true" />
      </a>
      <p class="text-muted-foreground flex items-center gap-1.5 text-xs">
        <Spinner class="size-3.5" aria-hidden="true" /> Waiting for you to finish…
      </p>
    </template>

    <template v-else-if="state.state === 'failed'">
      <Alert variant="destructive" data-testid="assistant-sign-in-error">
        <CircleAlert aria-hidden="true" />
        <AlertDescription>
          <p>Signing in to {{ providerName }} didn't finish. Please try again.</p>
          <details v-if="state.message" class="mt-1">
            <summary class="cursor-pointer text-xs underline">Details</summary>
            <p class="mt-1 text-xs break-words">{{ state.message }}</p>
          </details>
        </AlertDescription>
      </Alert>
      <div class="flex flex-wrap items-center gap-2">
        <Button size="sm" data-testid="assistant-sign-in-retry" @click="emit('retry')">
          Try again
        </Button>
        <Button
          variant="ghost"
          size="sm"
          data-testid="assistant-sign-in-dismiss"
          @click="emit('dismiss')"
        >
          Dismiss
        </Button>
      </div>
    </template>
  </div>
</template>
