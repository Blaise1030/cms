<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { FileDiff, type FileDiffMetadata } from "@pierre/diffs";
import { useTheme } from "@/lib/theme";

const props = defineProps<{ diff: FileDiffMetadata }>();

const theme = useTheme();
const host = ref<HTMLElement | null>(null);
let view: FileDiff | null = null;

function mount() {
  if (!host.value) return;
  view ??= new FileDiff({
    themeType: theme.resolved.value,
    diffStyle: "unified",
    overflow: "wrap",
    lineDiffType: "word",
  });
  view.render({ fileDiff: props.diff, containerWrapper: host.value });
}

onMounted(mount);
watch(() => props.diff, mount);
watch(theme.resolved, (type) => view?.setThemeType(type));
onBeforeUnmount(() => {
  view?.cleanUp();
  view = null;
});
</script>

<template>
  <div ref="host" class="bg-background overflow-hidden rounded-md border" data-testid="diff-view" />
</template>
