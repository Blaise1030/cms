<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { unifiedMergeView } from "@codemirror/merge";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

const props = defineProps<{ oldText: string; newText: string }>();

const host = ref<HTMLElement | null>(null);
let view: EditorView | null = null;

const theme = EditorView.theme(
  {
    "&": { backgroundColor: "transparent", fontSize: "12px", color: "inherit" },
    ".cm-scroller": { fontFamily: "var(--font-mono, ui-monospace, monospace)" },
    ".cm-gutters": { backgroundColor: "transparent", border: "none" },
  },
  { dark: true },
);

function mount() {
  view?.destroy();
  if (!host.value) return;
  view = new EditorView({
    parent: host.value,
    state: EditorState.create({
      doc: props.newText,
      extensions: [
        EditorState.readOnly.of(true),
        EditorView.editable.of(false),
        EditorView.lineWrapping,
        theme,
        unifiedMergeView({
          original: props.oldText,
          mergeControls: false,
          gutter: true,
          highlightChanges: true,
          syntaxHighlightDeletions: false,
        }),
      ],
    }),
  });
}

onMounted(mount);
watch(() => [props.oldText, props.newText], mount);
onBeforeUnmount(() => view?.destroy());
</script>

<template>
  <div ref="host" class="overflow-hidden rounded-md border" data-testid="diff-view" />
</template>
