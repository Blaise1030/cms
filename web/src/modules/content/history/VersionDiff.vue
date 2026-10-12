<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
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
      <div class="flex min-w-0 items-center gap-2">
        <span v-if="!canRestore && restoreReason" class="text-muted-foreground truncate text-xs">
          {{ restoreReason }}
        </span>
        <Button size="sm" :disabled="!canRestore" data-testid="restore-version" @click="$emit('restore')">
          <RotateCcw />
          Restore this version
        </Button>
      </div>
    </div>
    <Alert v-if="dirty">
      <AlertDescription>Comparing with the last saved version. Unsaved edits aren't included.</AlertDescription>
    </Alert>
    <p v-if="!diff" class="text-muted-foreground p-2 text-sm">No differences</p>
    <div v-else class="min-h-0 flex-1 overflow-auto">
      <DiffView :diff="diff" />
    </div>
  </div>
</template>
