<script setup lang="ts">
import { computed, defineAsyncComponent, useId } from "vue";
import { ArrowLeft, RotateCcw } from "@lucide/vue";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
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
      <Button variant="outline" size="sm" @click="$emit('back')">
        <ArrowLeft />
        Back to editor
      </Button>
      <Button
        size="sm"
        :disabled="!canRestore"
        :aria-describedby="showReason ? reasonId : undefined"
        data-testid="restore-version"
        @click="$emit('restore')"
      >
        <RotateCcw />
        Restore this version
      </Button>
    </div>
    <p v-if="showReason" :id="reasonId" class="text-muted-foreground text-xs">{{ restoreReason }}</p>
    <Alert v-if="dirty">
      <AlertDescription>Comparing with the last saved version. Unsaved edits aren't included.</AlertDescription>
    </Alert>
    <p v-if="!diff" class="text-muted-foreground p-2 text-sm">No differences</p>
    <div v-else class="min-h-0 flex-1 overflow-auto">
      <DiffView :diff="diff" />
    </div>
  </div>
</template>
