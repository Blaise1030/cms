<script setup lang="ts">
import { defineAsyncComponent, shallowRef, watch } from "vue";
import { Button } from "@/components/ui/button";
import type { ConflictChoice, ConflictFile } from "@/core/types";
import type { FileDiff } from "@/modules/checkpoints/diff";

const props = defineProps<{ file: ConflictFile; choice?: ConflictChoice }>();
const emit = defineEmits<{ choose: [choice: ConflictChoice] }>();
const DiffView = defineAsyncComponent(() => import("@/modules/checkpoints/DiffView.vue"));
const parsed = shallowRef<FileDiff | null>(null);
let parseRun = 0;
watch(() => props.file.patch, async (patch) => {
  const run = ++parseRun;
  parsed.value = null;
  if (!patch) return;
  const { parseUnifiedDiff } = await import("@/modules/checkpoints/diff");
  if (run === parseRun) parsed.value = parseUnifiedDiff(patch).files[0] ?? null;
}, { immediate: true });
</script>

<template>
  <div class="space-y-4" data-testid="conflict-file">
    <div>
      <p class="font-medium">{{ file.path.split("/").at(-1) }}</p>
      <p class="text-muted-foreground font-mono text-xs">{{ file.path }}</p>
    </div>
    <p class="text-muted-foreground text-sm">
      Choose the version to keep in this file. Keep both places your text before the latest text in each conflicting region.
      <span v-if="file.oursDeleted || file.theirsDeleted"> One version deleted this file.</span>
    </p>
    <div class="flex flex-wrap gap-2">
      <Button size="sm" :variant="choice === 'yours' ? 'default' : 'outline'" data-testid="keep-yours" @click="emit('choose', 'yours')">{{ file.oursDeleted ? 'Keep your deletion' : 'Keep yours' }}</Button>
      <Button size="sm" :variant="choice === 'latest' ? 'default' : 'outline'" data-testid="use-latest" @click="emit('choose', 'latest')">{{ file.theirsDeleted ? 'Use latest deletion' : 'Use latest' }}</Button>
      <Button size="sm" :variant="choice === 'both' ? 'default' : 'outline'" :disabled="file.binary || file.oursDeleted || file.theirsDeleted" data-testid="keep-both" @click="emit('choose', 'both')">Keep both</Button>
    </div>
    <p v-if="file.binary" class="text-muted-foreground text-sm">This file cannot be compared as text.</p>
    <DiffView v-else-if="parsed" :diff="parsed.meta" />
    <p v-else class="text-muted-foreground text-sm">No text comparison available for this file.</p>
  </div>
</template>
