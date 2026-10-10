<script setup lang="ts">
import { computed } from "vue";
import { toast } from "vue-sonner";
import { ExternalLink } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { errorMessage } from "@/lib/errors";
import { useChangesStore } from "./store";

const store = useChangesStore();
const status = computed(() => store.status);
/** Merging happens on GitHub: link the PR once it is out of draft. */
const reviewable = computed(
  () => status.value?.state === "open" || status.value?.state === "approved",
);

async function onReady() {
  const id = store.projectId;
  const branch = store.active?.branch;
  if (!id || !branch) return;
  try {
    await store.markReady(id, branch);
  } catch (e) {
    // An expired session shows the sign-in prompt instead.
    if (!store.needsSignIn) toast.error(errorMessage(e));
  }
}
</script>

<template>
  <Button
    v-if="status?.state === 'draft'"
    variant="outline"
    size="sm"
    class="h-7 shrink-0 px-2 text-xs"
    data-testid="ready-for-review"
    :disabled="store.markingReady"
    @click="onReady"
  >
    {{ store.markingReady ? "Marking ready…" : "Ready for review" }}
  </Button>
  <a
    v-else-if="status && reviewable"
    :href="status.url"
    target="_blank"
    rel="noopener noreferrer"
    class="text-muted-foreground hover:text-foreground flex h-7 shrink-0 items-center gap-1 rounded-md px-1.5 text-xs whitespace-nowrap underline-offset-2 hover:underline"
    title="Review and merge the pull request on GitHub"
    data-testid="open-on-github"
    ><ExternalLink class="size-3.5" />Open on GitHub</a
  >
</template>
