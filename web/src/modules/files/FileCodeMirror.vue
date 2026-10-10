<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { EditorState } from "@codemirror/state";
import { EditorView, lineNumbers } from "@codemirror/view";
import { oneDark } from "@codemirror/theme-one-dark";
import { languageForPath } from "./language";

const props = defineProps<{ code: string; path: string }>();

const host = ref<HTMLElement | null>(null);
let view: EditorView | null = null;

function extensions() {
  const lang = languageForPath(props.path);
  return [
    lineNumbers(),
    EditorState.readOnly.of(true),
    EditorView.editable.of(false),
    EditorView.lineWrapping,
    oneDark,
    ...(lang ? [lang] : []),
    // After oneDark so we strip its framed chrome and match the app background.
    EditorView.theme({
      "&": {
        height: "100%",
        maxHeight: "100%",
        fontSize: "12px",
        backgroundColor: "transparent !important",
        border: "none",
        outline: "none",
        boxShadow: "none",
      },
      "&.cm-focused": {
        outline: "none",
        border: "none",
        boxShadow: "none",
      },
      ".cm-scroller": {
        overflow: "auto",
        fontFamily: "var(--font-mono, ui-monospace, monospace)",
        backgroundColor: "transparent",
      },
      ".cm-gutters": {
        backgroundColor: "transparent",
        border: "none",
        borderRight: "none",
      },
      ".cm-gutterElement": {
        backgroundColor: "transparent",
      },
      ".cm-activeLineGutter": {
        backgroundColor: "transparent",
      },
      ".cm-activeLine": {
        backgroundColor: "transparent",
      },
      ".cm-content": {
        padding: "8px 0",
        caretColor: "transparent",
      },
    }),
  ];
}

function mount() {
  if (!host.value) return;
  view?.destroy();
  view = new EditorView({
    parent: host.value,
    state: EditorState.create({
      doc: props.code,
      extensions: extensions(),
    }),
  });
}

onMounted(mount);
watch(() => [props.code, props.path] as const, mount);
onBeforeUnmount(() => {
  view?.destroy();
  view = null;
});
</script>

<template>
  <div
    ref="host"
    class="file-codemirror h-full min-h-0 w-full overflow-hidden"
    data-testid="file-content"
  />
</template>

<style scoped>
.file-codemirror :deep(.cm-editor) {
  height: 100%;
  border: none !important;
  outline: none !important;
  box-shadow: none !important;
  background: transparent !important;
}
.file-codemirror :deep(.cm-scroller) {
  background: transparent !important;
}
.file-codemirror :deep(.cm-gutters) {
  border: none !important;
  background: transparent !important;
}
</style>
