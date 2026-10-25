<script setup lang="ts">
import { computed } from "vue";
import { Rocket } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { errorMessage } from "@orso/utils";
import { useChangesStore } from "./store";

const props = defineProps<{ projectId: string; repo: string | null }>();
const store = useChangesStore();

/** Offered for projects opened from GitHub, until the active change is live. */
const visible = computed(
  () => props.repo !== null && store.active !== null && store.status?.state !== "merged",
);

async function onPublish() {
  const branch = store.active?.branch;
  if (!branch) return;
  try {
    await store.publish(props.projectId, branch);
    toast.success("It's live.");
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
</script>

<template>
  <Button
    v-if="visible"
    size="sm"
    class="h-7 gap-1.5 px-2.5 text-xs"
    data-testid="publish-change"
    :disabled="store.publishing"
    @click="onPublish"
  >
    <Spinner v-if="store.publishing" class="size-4" />
    <Rocket v-else class="size-4" />
    {{ store.publishing ? "Publishing…" : "Publish" }}
  </Button>
</template>
