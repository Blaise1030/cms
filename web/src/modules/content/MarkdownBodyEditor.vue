<script setup lang="ts">
import { computed, inject, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { Extension, type Range } from "@tiptap/core";
import { Editor, EditorContent } from "@tiptap/vue-3";
import Suggestion, { type SuggestionKeyDownProps, type SuggestionProps } from "@tiptap/suggestion";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { markdown as markdownLanguage } from "@codemirror/lang-markdown";
import { Button } from "@/components/ui/button";
import { canRoundTrip, markdownExtensions } from "./markdown";
import ImageDialog, { IMAGE_REGISTRY, markdownImage, type PendingImage } from "./ImageDialog.vue";

/**
 * Body editor for Markdown/MDX entries. `body` is the draft body and `original` the loaded one.
 * Rich mode (Tiptap) is offered only when `rich` is set for a Markdown entry and the text can be
 * represented by the configured extensions; everything else is edited raw (CodeMirror).
 * The text is kept byte-for-byte until the writer edits it: opening or toggling never reserializes.
 */
/** `resolveImage` maps an image link to the URL the rich editor displays; the text keeps the link. */
const props = defineProps<{ body: string; original: string; kind: "markdown" | "mdx"; rich: boolean; resolveImage?: (src: string) => string }>();
const emit = defineEmits<{ change: [value: { body: string; dirty: boolean }] }>();

const richCapable = computed(() => props.rich && props.kind === "markdown");
/** The current body text; only replaced by a real edit or an external reset. */
let text = props.body;
const mode = ref<"rich" | "raw">("raw");
/** Shown when the text holds Markdown the rich editor cannot represent. */
const unsupported = ref(false);
const richEditor = shallowRef<Editor | null>(null);
const rawHost = ref<HTMLElement | null>(null);
let rawView: EditorView | null = null;

function edited(next: string) {
  text = next;
  emit("change", { body: next, dirty: next !== props.original });
}

// ---- Images ----------------------------------------------------------------------------------
/** The draft that uploads inserted images on Save. */
const registry = inject(IMAGE_REGISTRY, null);
const imageOpen = ref(false);
const imageFile = shallowRef<File | null>(null);
let placeImage: ((image: PendingImage) => void) | null = null;
/** Open the one preview/alt dialog; `place` inserts the image once alt text is given. */
function requestImage(file: File | null, place: (image: PendingImage) => void) {
  if (imageOpen.value) return;
  imageFile.value = file; placeImage = place; imageOpen.value = true;
}
function imageInserted(image: PendingImage) {
  registry?.add(image);
  placeImage?.(image); placeImage = null;
}
function insertRich(editor: Editor, image: PendingImage, range?: Range) {
  const chain = editor.chain().focus();
  if (range) chain.deleteRange(range);
  // The Markdown parser keeps `\\` in alt text as written, so store backslashes the way it would
  // read them back from `escapeAlt` output.
  chain.setImage({ src: image.previewUrl, alt: image.alt.replace(/\\/g, "\\\\") }).run();
}
function insertRaw(image: PendingImage) {
  if (!rawView) return;
  rawView.dispatch(rawView.state.replaceSelection(markdownImage(image.alt, image.previewUrl)), { userEvent: "input" });
  rawView.focus();
}
function insertHere(image: PendingImage) {
  if (mode.value === "rich" && richEditor.value) insertRich(richEditor.value, image);
  else insertRaw(image);
}
/** Paste or drop of a file in either mode goes through the dialog instead of the editor. */
function transfer(event: ClipboardEvent | DragEvent) {
  const data = "clipboardData" in event ? event.clipboardData : event.dataTransfer;
  const file = data?.files?.[0];
  if (!file) return;
  event.preventDefault(); event.stopPropagation();
  requestImage(file, insertHere);
}
function dragOver(event: DragEvent) { if (event.dataTransfer?.types?.includes("Files")) event.preventDefault(); }

// ---- Toolbar and slash actions -------------------------------------------------------------
function insertImage(editor: Editor, range?: Range) {
  requestImage(null, image => insertRich(editor, image, range));
}
function insertLink(editor: Editor, range?: Range) {
  const href = window.prompt("Link URL")?.trim();
  if (!href) return;
  const chain = editor.chain().focus();
  if (range) chain.deleteRange(range);
  if (range || editor.state.selection.empty) chain.insertContent({ type: "text", text: href, marks: [{ type: "link", attrs: { href } }] }).run();
  else chain.extendMarkRange("link").setLink({ href }).run();
}

type Action = { id: string; label: string; run: (editor: Editor, range?: Range) => void };
const block = (fn: (c: ReturnType<Editor["chain"]>) => ReturnType<Editor["chain"]>) => (editor: Editor, range?: Range) => {
  const chain = editor.chain().focus();
  if (range) chain.deleteRange(range);
  fn(chain).run();
};
const blockActions: Action[] = [
  { id: "heading", label: "Heading", run: block(c => c.toggleHeading({ level: 2 })) },
  { id: "bullet-list", label: "Bullet list", run: block(c => c.toggleBulletList()) },
  { id: "ordered-list", label: "Numbered list", run: block(c => c.toggleOrderedList()) },
  { id: "link", label: "Link", run: insertLink },
  { id: "quote", label: "Quote", run: block(c => c.toggleBlockquote()) },
  { id: "code-block", label: "Code block", run: block(c => c.toggleCodeBlock()) },
  { id: "image", label: "Image", run: insertImage },
];
const inlineActions: Action[] = [
  { id: "bold", label: "Bold", run: block(c => c.toggleBold()) },
  { id: "italic", label: "Italic", run: block(c => c.toggleItalic()) },
  { id: "code", label: "Inline code", run: block(c => c.toggleCode()) },
];
function runTool(action: Action) { if (richEditor.value) action.run(richEditor.value); }

const slash = shallowRef<{ items: Action[]; command: (item: Action) => void; active: number } | null>(null);
function slashKey({ event }: SuggestionKeyDownProps) {
  const s = slash.value;
  if (!s || !s.items.length) return false;
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    const step = event.key === "ArrowDown" ? 1 : -1;
    slash.value = { ...s, active: (s.active + step + s.items.length) % s.items.length };
    return true;
  }
  if (event.key === "Enter") { s.command(s.items[s.active]!); return true; }
  if (event.key === "Escape") { slash.value = null; return true; }
  return false;
}
const SlashCommands = Extension.create({
  name: "slashCommands",
  addProseMirrorPlugins() {
    return [Suggestion<Action, Action>({
      editor: this.editor,
      char: "/",
      items: ({ query }) => blockActions.filter(a => a.label.toLowerCase().includes(query.toLowerCase())),
      command: ({ editor, range, props: action }) => action.run(editor as Editor, range),
      render: () => {
        const show = (p: SuggestionProps<Action, Action>) => { slash.value = { items: p.items, command: p.command, active: 0 }; };
        return { onStart: show, onUpdate: show, onExit: () => { slash.value = null; }, onKeyDown: slashKey };
      },
    })];
  },
});

// ---- Modes ---------------------------------------------------------------------------------
function destroyRich() { richEditor.value?.destroy(); richEditor.value = null; slash.value = null; }
function destroyRaw() { rawView?.destroy(); rawView = null; }
function mountRich() {
  destroyRich();
  // The serializer drops the final newline(s); keep the file's own ending after an edit.
  const ending = /\n*$/.exec(text)![0];
  // Loading content does not fire `onUpdate`; only the writer's transactions do.
  richEditor.value = new Editor({
    extensions: [...markdownExtensions(props.resolveImage), SlashCommands],
    content: text,
    contentType: "markdown",
    onUpdate: ({ editor }) => edited(editor.getMarkdown().replace(/\n+$/, "") + ending),
  });
}
function mountRaw() {
  destroyRaw();
  if (!rawHost.value) return;
  rawView = new EditorView({
    parent: rawHost.value,
    state: EditorState.create({
      doc: text,
      extensions: [
        markdownLanguage(),
        EditorView.lineWrapping,
        EditorView.updateListener.of(update => { if (update.docChanged) edited(update.state.doc.toString()); }),
        EditorView.theme({
          "&": { fontSize: "13px", minHeight: "12rem" },
          "&.cm-focused": { outline: "none" },
          ".cm-scroller": { fontFamily: "var(--font-mono, ui-monospace, monospace)" },
        }),
      ],
    }),
  });
}
async function setMode(next: "rich" | "raw") {
  if (next === "rich") {
    if (!richCapable.value) return;
    // Refuse rather than parse: the rich editor would drop what it cannot represent.
    if (!canRoundTrip(text)) { unsupported.value = true; return; }
    unsupported.value = false;
    destroyRaw(); mode.value = "rich"; mountRich();
  } else {
    destroyRich(); mode.value = "raw";
    await nextTick(); mountRaw();
  }
}
/** Start over from `text`: rich when allowed and representable, otherwise raw. */
async function reset() {
  destroyRich(); destroyRaw();
  const rich = richCapable.value && canRoundTrip(text);
  unsupported.value = richCapable.value && !rich;
  mode.value = rich ? "rich" : "raw";
  await nextTick();
  if (rich) mountRich(); else mountRaw();
}
onMounted(() => { void reset(); });
// An external draft reset (Reload, discard on navigation) replaces the text.
watch(() => props.body, body => { if (body !== text) { text = body; void reset(); } });
watch(richCapable, () => void reset());
onBeforeUnmount(() => { destroyRich(); destroyRaw(); });

defineExpose({ richEditor: () => richEditor.value, mode });
</script>

<template>
  <div class="mb-3" data-testid="body-editor" @paste.capture="transfer" @drop.capture="transfer" @dragover.capture="dragOver">
    <div class="mb-1 flex flex-wrap items-center gap-1">
      <span class="mr-auto text-xs font-medium">Body</span>
      <template v-if="richCapable">
        <Button type="button" size="sm" :variant="mode === 'rich' ? 'secondary' : 'ghost'" data-testid="body-mode-rich" @click="setMode('rich')">Rich</Button>
        <Button type="button" size="sm" :variant="mode === 'raw' ? 'secondary' : 'ghost'" data-testid="body-mode-raw" @click="setMode('raw')">Raw Markdown</Button>
      </template>
      <span v-else class="text-xs text-muted-foreground">{{ kind === "mdx" ? "MDX (raw)" : "Raw Markdown" }}</span>
    </div>
    <p v-if="unsupported" role="status" class="mb-1 text-xs text-muted-foreground" data-testid="body-unsupported">
      This body has Markdown the rich editor cannot represent (such as HTML or MDX), so it stays in Raw view.
    </p>
    <template v-if="mode === 'rich' && richEditor">
      <div class="mb-1 flex flex-wrap gap-1" role="toolbar" aria-label="Formatting" data-testid="body-toolbar">
        <Button v-for="action in [...blockActions, ...inlineActions]" :key="action.id" type="button" size="sm" variant="outline" :data-testid="`body-tool-${action.id}`" @mousedown.prevent @click="runTool(action)">{{ action.label }}</Button>
      </div>
      <div class="relative">
        <EditorContent :editor="richEditor" data-testid="body-rich" class="content-rich min-h-48 rounded-md border bg-background px-3 py-2 text-sm" />
        <ul v-if="slash && slash.items.length" class="absolute left-2 z-10 mt-1 w-48 rounded-md border bg-popover p-1 text-sm shadow" role="listbox" data-testid="slash-menu">
          <li v-for="(item, index) in slash.items" :key="item.id" role="option" :aria-selected="index === slash.active" data-testid="slash-item" class="cursor-pointer rounded px-2 py-1" :class="{ 'bg-muted': index === slash.active }" @mousedown.prevent="slash.command(item)">{{ item.label }}</li>
        </ul>
      </div>
    </template>
    <div v-if="mode === 'raw'" class="mb-1 flex gap-1">
      <Button type="button" size="sm" variant="outline" data-testid="body-raw-image" @click="requestImage(null, insertRaw)">Image</Button>
    </div>
    <div v-show="mode === 'raw'" ref="rawHost" data-testid="body-raw" class="rounded-md border bg-background" />
    <ImageDialog v-model:open="imageOpen" :file="imageFile" @insert="imageInserted" />
  </div>
</template>

<style scoped>
.content-rich :deep(.ProseMirror) { outline: none; min-height: 11rem; }
.content-rich :deep(h1) { font-size: 1.5rem; font-weight: 600; }
.content-rich :deep(h2) { font-size: 1.25rem; font-weight: 600; }
.content-rich :deep(h3) { font-size: 1.1rem; font-weight: 600; }
.content-rich :deep(ul) { list-style: disc; padding-left: 1.5rem; }
.content-rich :deep(ol) { list-style: decimal; padding-left: 1.5rem; }
.content-rich :deep(blockquote) { border-left: 3px solid var(--border); padding-left: 0.75rem; }
.content-rich :deep(pre) { background: var(--muted); border-radius: 0.375rem; padding: 0.5rem; font-family: var(--font-mono, ui-monospace, monospace); }
.content-rich :deep(code) { font-family: var(--font-mono, ui-monospace, monospace); }
.content-rich :deep(a) { text-decoration: underline; }
.content-rich :deep(img) { max-width: 100%; }
.content-rich :deep(p), .content-rich :deep(h1), .content-rich :deep(h2), .content-rich :deep(blockquote), .content-rich :deep(pre) { margin: 0.5rem 0; }
</style>
