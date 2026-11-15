<script setup lang="ts">
// The sign-in step for one assistant, shown under its row while a login is
// pending. Three shapes: waiting in the browser, asking for a pasted code, and
// "it didn't finish" with a retry. The panel is presentational: the parent owns
// the store calls, so every button is an event.
import { computed, ref } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UCard from "@nuxt/ui/components/Card.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UCollapsible from "@nuxt/ui/components/Collapsible.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UInput from "@nuxt/ui/components/Input.vue";
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
  <UCard
    v-if="state.state === 'browser' || state.state === 'code' || state.state === 'failed'"
    variant="outline"
    :ui="{ body: 'flex flex-col gap-2 p-3 text-sm sm:p-3' }"
    role="group"
    :aria-label="`Sign in to ${providerName}`"
    :data-state="state.state"
    data-testid="assistant-sign-in"
  >
    <template v-if="state.state === 'browser'">
      <p class="text-highlighted font-medium">Finish signing in to {{ providerName }}</p>
      <p class="text-muted">Complete the sign-in in the browser window orso opened.</p>
      <p v-if="state.message" class="text-muted">{{ state.message }}</p>
      <div class="flex flex-wrap items-center gap-2">
        <UButton
          v-if="state.url"
          :to="state.url"
          target="_blank"
          rel="noopener noreferrer"
          color="neutral"
          variant="outline"
          size="sm"
          icon="i-hugeicons-link-square-02"
          label="Open the page again"
          data-testid="assistant-sign-in-open"
        />
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          label="Cancel"
          data-testid="assistant-sign-in-cancel"
          @click="emit('cancel')"
        />
      </div>
      <p class="text-muted flex items-center gap-1.5 text-xs">
        <UIcon name="i-hugeicons-loading-03" class="size-3.5 animate-spin" aria-hidden="true" />
        Waiting for you to finish…
      </p>
    </template>

    <template v-else-if="state.state === 'code'">
      <p class="text-highlighted font-medium">Enter the code to sign in to {{ providerName }}</p>
      <p v-if="state.message" class="text-muted">{{ state.message }}</p>
      <form class="flex flex-wrap items-center gap-2" @submit.prevent="submit">
        <UInput
          v-model="code"
          class="max-w-52"
          aria-label="Sign-in code"
          placeholder="Paste the code"
          autocomplete="one-time-code"
          data-testid="assistant-sign-in-code"
        />
        <UButton
          type="submit"
          size="sm"
          label="Continue"
          :disabled="!canSubmit"
          data-testid="assistant-sign-in-submit"
        />
        <UButton
          type="button"
          color="neutral"
          variant="ghost"
          size="sm"
          label="Cancel"
          data-testid="assistant-sign-in-cancel"
          @click="emit('cancel')"
        />
      </form>
      <UButton
        v-if="state.url"
        :to="state.url"
        target="_blank"
        rel="noopener noreferrer"
        variant="link"
        color="neutral"
        size="xs"
        class="self-start px-0 underline"
        trailing-icon="i-hugeicons-link-square-02"
        label="Open the sign-in page"
        data-testid="assistant-sign-in-open"
      />
      <p class="text-muted flex items-center gap-1.5 text-xs">
        <UIcon name="i-hugeicons-loading-03" class="size-3.5 animate-spin" aria-hidden="true" />
        Waiting for you to finish…
      </p>
    </template>

    <template v-else-if="state.state === 'failed'">
      <UAlert
        color="error"
        variant="subtle"
        icon="i-hugeicons-alert-circle"
        data-testid="assistant-sign-in-error"
      >
        <template #description>
          <p>Signing in to {{ providerName }} didn't finish. Please try again.</p>
          <UCollapsible v-if="state.message" :unmount-on-hide="false" class="mt-1">
            <UButton
              variant="link"
              color="neutral"
              size="xs"
              class="px-0 underline"
              label="Details"
              data-testid="assistant-details-toggle"
            />
            <template #content>
              <p class="mt-1 text-xs break-words" data-testid="assistant-details">{{ state.message }}</p>
            </template>
          </UCollapsible>
        </template>
      </UAlert>
      <div class="flex flex-wrap items-center gap-2">
        <UButton
          size="sm"
          label="Try again"
          data-testid="assistant-sign-in-retry"
          @click="emit('retry')"
        />
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          label="Dismiss"
          data-testid="assistant-sign-in-dismiss"
          @click="emit('dismiss')"
        />
      </div>
    </template>
  </UCard>
</template>
