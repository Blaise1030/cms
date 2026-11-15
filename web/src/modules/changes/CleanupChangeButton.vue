<script setup lang="ts">
import { computed } from "vue";
import { toast } from "@/lib/toast";
import UButton from "@nuxt/ui/components/Button.vue";
import { errorMessage } from "@orso/utils";
import { useChangesStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useChangesStore();

/** Offered once the active change's pull request is merged on GitHub. */
const visible = computed(() => store.active !== null && store.status?.state === "merged");

async function onCleanup() {
  const branch = store.active?.branch;
  if (!branch) return;
  try {
    const { mainUpdated } = await store.cleanup(props.projectId, branch);
    if (!mainUpdated) {
      toast.warning("Cleaned up, but main couldn't be updated from GitHub — it may be out of date");
    }
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
</script>

<template>
  <UButton
    v-if="visible"
    color="neutral"
    variant="outline"
    size="sm"
    class="shrink-0"
    title="Delete this change's local branch and return to the updated main"
    data-testid="cleanup-change"
    :disabled="store.cleaningUp"
    :label="store.cleaningUp ? 'Cleaning up…' : 'Clean up'"
    @click="onCleanup"
  />
</template>
