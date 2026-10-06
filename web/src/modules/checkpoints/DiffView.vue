<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { FileDiff, type FileDiffMetadata } from "@pierre/diffs";

const props = defineProps<{ diff: FileDiffMetadata }>();

const host = ref<HTMLElement | null>(null);
let view: FileDiff | null = null;

function mount() {
  if (!host.value) return;
  view ??= new FileDiff({
    themeType: "dark",
    diffStyle: "unified",
    overflow: "wrap",
    lineDiffType: "word",
  });
  view.render({ fileDiff: props.diff, containerWrapper: host.value });
}

onMounted(mount);
watch(() => props.diff, mount);
onBeforeUnmount(() => {
  view?.cleanUp();
  view = null;
});
</script>

<template>
  <div ref="host" class="overflow-hidden rounded-md border" data-testid="diff-view" />
</template>
