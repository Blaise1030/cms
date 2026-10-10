<script setup lang="ts">
import { computed } from "vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import { errorMessage } from "@/lib/errors";
import { useChangesStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useChangesStore();

/** Offered once the active change's pull request is merged on GitHub. */
const visible = computed(() => store.active !== null && store.status?.state === "merged");

async function onCleanup() {
  const branch = store.active?.branch;
  if (!branch) return;
  try {
    await store.cleanup(props.projectId, branch);
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
</script>

<template>
  <Button
    v-if="visible"
    variant="outline"
    size="sm"
    class="h-7 shrink-0 px-2 text-xs"
    title="Delete this change's local branch and return to the updated main"
    data-testid="cleanup-change"
    :disabled="store.cleaningUp"
    @click="onCleanup"
    >{{ store.cleaningUp ? "Cleaning up…" : "Clean up" }}</Button
  >
</template>
