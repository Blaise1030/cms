<script setup lang="ts">
import { defineAsyncComponent, shallowRef, watch } from "vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import type { ConflictFile } from "@/core/types";
import type { FileDiff } from "@/modules/checkpoints/diff";

/** The text comparison behind a combine choice, collapsed by default: writers decide from the previews. */
const props = defineProps<{ file: ConflictFile }>();
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
  <details class="group" data-testid="conflict-code">
    <summary class="text-muted hover:text-default flex cursor-pointer list-none items-center gap-1.5 text-sm select-none">
      <UIcon name="i-hugeicons-arrow-right-01" class="size-4 transition-transform group-open:rotate-90" />
      Show code
      <span class="font-mono text-xs">{{ file.path }}</span>
    </summary>
    <div class="mt-3">
      <p v-if="file.binary" class="text-muted text-sm">This file can't be shown as text.</p>
      <template v-else-if="parsed">
        <p class="text-muted mb-2 text-xs">Red lines are your version, green lines are the live site's.</p>
        <DiffView :diff="parsed.meta" />
      </template>
      <p v-else class="text-muted text-sm">No text to compare for this file.</p>
    </div>
  </details>
</template>
