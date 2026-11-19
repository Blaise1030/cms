<script setup lang="ts">
import { computed } from "vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import { getLatest } from "./getLatest";
import { useChangesStore } from "./store";

defineProps<{ projectId: string }>();
const store = useChangesStore();

/** Null while the daemon hasn't said — keep it enabled rather than wrongly disabling. */
const nothingToSync = computed(() => store.syncNeeded === false);
const title = computed(() => (nothingToSync.value ? "Already up to date" : "Sync with live site"));
const disabled = computed(
  () => store.updating || store.switching || store.creating || nothingToSync.value,
);
</script>

<template>
  <UTooltip :text="title" :delay-duration="300">
    <!-- A disabled button gets no pointer events; the span keeps the tooltip working. -->
    <span class="inline-flex">
      <UButton
        color="neutral"
        variant="outline"
        size="sm"
        square
        icon="i-hugeicons-cloud-download"
        :loading="store.updating"
        data-testid="get-latest"
        :aria-label="title"
        :disabled="disabled"
        @click="getLatest(projectId)"
      />
    </span>
  </UTooltip>
</template>
