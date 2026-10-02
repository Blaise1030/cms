<script setup lang="ts">
import { onBeforeUnmount, watch } from "vue";
import { EditorContent, useEditor, type JSONContent } from "@tiptap/vue-3";
import StarterKit from "@tiptap/starter-kit";
import { Placeholder } from "@tiptap/extensions";
import { usePromptInput } from "@/components/ai-elements/prompt-input/context";

const props = withDefaults(defineProps<{ placeholder?: string; ariaLabel?: string }>(), {
  placeholder: "",
  ariaLabel: "Message",
});

const { textInput, setTextInput } = usePromptInput();

// The agent receives plain text, so keep the editor plain: paragraphs, line breaks and undo only.
// Formatting marks and markdown input rules would be silently dropped by getText().
const extensions = [
  StarterKit.configure({
    blockquote: false,
    bold: false,
    bulletList: false,
    code: false,
    codeBlock: false,
    heading: false,
    horizontalRule: false,
    italic: false,
    link: false,
    listItem: false,
    listKeymap: false,
    orderedList: false,
    strike: false,
    underline: false,
    trailingNode: false,
  }),
  Placeholder.configure({ placeholder: () => props.placeholder }),
];

function textOf(): string {
  return editor.value?.getText({ blockSeparator: "\n" }) ?? "";
}

function docFrom(text: string): JSONContent {
  return {
    type: "doc",
    content: text.split("\n").map((line) => ({
      type: "paragraph",
      content: line ? [{ type: "text", text: line }] : [],
    })),
  };
}

const editor = useEditor({
  extensions,
  content: docFrom(textInput.value),
  editorProps: {
    attributes: {
      role: "textbox",
      "aria-multiline": "true",
      "aria-label": props.ariaLabel,
      "data-testid": "composer-editor",
      class: "max-h-48 min-h-12 overflow-y-auto px-3 pt-3 pb-1 text-sm outline-none",
    },
    // Enter sends (like the textarea it replaces); Shift+Enter starts a new line. IME composition
    // is left alone so confirming a candidate doesn't submit.
    handleKeyDown(view, event) {
      if (event.key !== "Enter" || event.shiftKey || event.isComposing) return false;
      const form = view.dom.closest("form");
      const submit = form?.querySelector<HTMLButtonElement>('button[type="submit"]');
      if (!form || submit?.disabled) return true;
      form.requestSubmit();
      return true;
    },
  },
  onUpdate: () => setTextInput(textOf()),
});

// The prompt context clears the text on submit and restores it if sending fails.
watch(textInput, (value) => {
  if (editor.value && value !== textOf()) {
    editor.value.commands.setContent(docFrom(value), { emitUpdate: false });
  }
});

onBeforeUnmount(() => editor.value?.destroy());

defineExpose({
  editor,
  focus: () => editor.value?.commands.focus("end"),
});
</script>

<template>
  <EditorContent :editor="editor" class="composer-editor w-full min-w-0" />
</template>

<style>
.composer-editor .tiptap p.is-editor-empty:first-child::before {
  content: attr(data-placeholder);
  float: left;
  height: 0;
  pointer-events: none;
  color: var(--muted-foreground);
}
</style>
