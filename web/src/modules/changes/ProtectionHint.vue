<script setup lang="ts">
import { ShieldAlert, X } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
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
  <Alert
    v-if="changes.protectionHint"
    class="absolute top-full right-0 z-50 mt-2 w-80 pr-9 shadow-md"
    data-testid="protection-hint"
  >
    <ShieldAlert />
    <AlertTitle>Protect main so merges need review</AlertTitle>
    <AlertDescription>
      <p>
        Anyone with push access can merge this change straight to <code>main</code>. Add a branch
        protection rule that requires a pull request review.
      </p>
      <p class="flex gap-3">
        <a
          class="underline underline-offset-2"
          data-testid="protection-hint-settings"
          :href="changes.protectionHint.settingsUrl"
          target="_blank"
          rel="noopener noreferrer"
          >Branch settings</a
        >
        <a
          class="underline underline-offset-2"
          data-testid="protection-hint-docs"
          :href="changes.protectionHint.docsUrl"
          target="_blank"
          rel="noopener noreferrer"
          >How to</a
        >
      </p>
    </AlertDescription>
    <Button
      variant="ghost"
      size="icon"
      class="absolute top-2 right-2 size-6"
      aria-label="Dismiss"
      data-testid="protection-hint-dismiss"
      @click="onDismiss"
    >
      <X />
    </Button>
  </Alert>
</template>
