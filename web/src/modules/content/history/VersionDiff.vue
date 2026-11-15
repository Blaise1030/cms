<script setup lang="ts">
import { computed, defineAsyncComponent, useId } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import { versionDiff } from "./history";

// Lazy: the Pierre renderers bring Shiki along, which must stay out of the precached app shell.
const DiffView = defineAsyncComponent(() => import("@/modules/checkpoints/DiffView.vue"));

const props = defineProps<{
  path: string;
  oldSource: string;
  newSource: string;
  dirty: boolean;
  canRestore: boolean;
  restoreReason?: string;
}>();
defineEmits<{ back: []; restore: [] }>();

const reasonId = useId();
const showReason = computed(() => !props.canRestore && !!props.restoreReason);
const diff = computed(() =>
  props.oldSource === props.newSource ? null : versionDiff(props.path, props.oldSource, props.newSource),
);
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-2 p-2" data-testid="version-diff">
    <div class="flex shrink-0 items-center justify-between gap-2">
      <UButton type="button" color="neutral" variant="outline" size="sm" icon="i-hugeicons-arrow-left-01" label="Back to editor" @click="$emit('back')" />
      <UButton
        type="button"
        size="sm"
        icon="i-hugeicons-arrow-turn-backward"
        label="Restore this version"
        :disabled="!canRestore"
        :aria-describedby="showReason ? reasonId : undefined"
        data-testid="restore-version"
        @click="$emit('restore')"
      />
    </div>
    <p v-if="showReason" :id="reasonId" class="text-muted text-xs">{{ restoreReason }}</p>
    <UAlert
      v-if="dirty"
      color="neutral"
      variant="outline"
      description="Comparing with the last saved version. Unsaved edits aren't included."
    />
    <p v-if="!diff" class="text-muted p-2 text-sm">No differences</p>
    <div v-else class="min-h-0 flex-1 overflow-auto">
      <DiffView :diff="diff" />
    </div>
  </div>
</template>
