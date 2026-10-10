<script setup lang="ts">
import { computed } from "vue";
import { ExternalLink } from "@lucide/vue";
import type { ChecksState } from "@/core/types";
import { prStatusLabel } from "./prStatus";
import { useChangesStore } from "./store";

const store = useChangesStore();
const status = computed(() => store.status);

const DOT: Record<ChecksState, string> = {
  none: "bg-muted-foreground/50",
  pending: "bg-amber-500 animate-pulse",
  passing: "bg-emerald-500",
  failing: "bg-red-500",
};
</script>

<template>
  <template v-if="status">
    <a
      :href="status.url"
      target="_blank"
      rel="noopener noreferrer"
      class="text-muted-foreground hover:text-foreground flex h-7 shrink-0 items-center gap-1.5 rounded-md border px-2 text-xs whitespace-nowrap"
      :title="`Pull request #${status.number}`"
      data-testid="pr-status"
      :data-state="status.state"
      :data-checks="status.checks"
      ><span class="size-1.5 shrink-0 rounded-full" :class="DOT[status.checks]" aria-hidden="true" />{{
        prStatusLabel(status)
      }}</a
    >
    <a
      v-if="status.previewUrl"
      :href="status.previewUrl"
      target="_blank"
      rel="noopener noreferrer"
      class="text-muted-foreground hover:text-foreground flex h-7 shrink-0 items-center gap-1 rounded-md px-1.5 text-xs whitespace-nowrap underline-offset-2 hover:underline"
      title="Open the deployed preview"
      data-testid="preview-deploy-link"
    >
      <ExternalLink class="size-3.5" />
      Preview
    </a>
  </template>
</template>
