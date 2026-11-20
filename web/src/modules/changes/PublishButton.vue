<script setup lang="ts">
import { computed } from "vue";
import { toast } from "@/lib/toast";
import UButton from "@nuxt/ui/components/Button.vue";
import { errorMessage } from "@orso/utils";
import { useChangesStore } from "./store";

const props = defineProps<{ projectId: string; repo: string | null }>();
const store = useChangesStore();

/** Offered for projects opened from GitHub, until the active change is live. */
const visible = computed(
  () => props.repo !== null && store.active !== null && store.status?.state !== "merged",
);

/** GitHub's answer: conflicts, branch protection, or being behind `main` stop a merge. */
const canMerge = computed(() => {
  const status = store.status;
  if (!status) return true;
  if (status.state === "closed") return false;
  // A reviewer's send-back holds the change until it is updated, whatever GitHub says about merging.
  if (status.state === "changes_requested") return false;
  // Publish marks a draft ready itself; GitHub reports drafts as unmergeable until then.
  if (status.state === "draft") return true;
  if (status.mergeable === false) return false;
  return !["dirty", "blocked", "behind"].includes(status.mergeableState ?? "");
});

/** Explained on hover, so a disabled Publish still says why. */
const disabledReason = computed(() => {
  if (store.status?.state === "closed") return "This change was closed without publishing.";
  if (store.status?.state === "changes_requested") return "A reviewer sent this change back.";
  return "This change can't be published yet.";
});

async function onPublish() {
  const branch = store.active?.branch;
  if (!branch) return;
  try {
    // Publish reports "live" only once the site's deploy has shipped (see the store).
    await store.publish(props.projectId, branch);
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
</script>

<template>
  <UButton
    v-if="visible"
    size="sm"
    icon="i-hugeicons-rocket-01"
    data-testid="publish-change"
    :title="canMerge ? undefined : disabledReason"
    :loading="store.publishing"
    :disabled="store.publishing || !canMerge"
    :label="store.publishing ? 'Publishing…' : 'Publish'"
    @click="onPublish"
  />
</template>
