<script setup lang="ts">
import { toast } from "@/lib/toast";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import ULink from "@nuxt/ui/components/Link.vue";
import { errorMessage } from "@orso/utils";
import { useChangesStore } from "@/modules/changes/store";

const props = defineProps<{ projectId: string }>();
const changes = useChangesStore();

async function onDismiss() {
  try {
    await changes.dismissProtectionHint(props.projectId);
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
</script>

<template>
  <UAlert
    v-if="changes.protectionHint"
    color="neutral"
    variant="outline"
    icon="i-hugeicons-shield-alert"
    title="Protect main so merges need review"
    close
    class="bg-default absolute top-full right-0 z-50 mt-2 w-80 shadow-md"
    data-testid="protection-hint"
  >
    <template #description>
      <p>
        Anyone with push access can merge this change straight to <code>main</code>. Add a branch
        protection rule that requires a pull request review.
      </p>
      <p class="mt-2 flex gap-3">
        <ULink
          class="text-highlighted underline underline-offset-2"
          data-testid="protection-hint-settings"
          :to="changes.protectionHint.settingsUrl"
          target="_blank"
          rel="noopener noreferrer"
          >Branch settings</ULink
        >
        <ULink
          class="text-highlighted underline underline-offset-2"
          data-testid="protection-hint-docs"
          :to="changes.protectionHint.docsUrl"
          target="_blank"
          rel="noopener noreferrer"
          >How to</ULink
        >
      </p>
    </template>
    <template #close>
      <UButton
        color="neutral"
        variant="link"
        size="sm"
        icon="i-hugeicons-cancel-01"
        aria-label="Dismiss"
        data-testid="protection-hint-dismiss"
        @click="onDismiss"
      />
    </template>
  </UAlert>
</template>
