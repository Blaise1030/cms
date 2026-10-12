<script setup lang="ts">
import { computed, inject, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { Extension, type Range } from "@tiptap/core";
import { Editor, EditorContent } from "@tiptap/vue-3";
import Suggestion, { type SuggestionKeyDownProps, type SuggestionProps } from "@tiptap/suggestion";
import { EditorState } from "@codemirror/state";
import { Placeholder } from "@tiptap/extensions";
import type { ResolvedPos } from "@tiptap/pm/model";
import { NodeSelection, TextSelection } from "@tiptap/pm/state";
import { EditorView, placeholder } from "@codemirror/view";
import { markdown as markdownLanguage } from "@codemirror/lang-markdown";
import { Bold, ChevronDown, Code, GripVertical, Heading, ImagePlus, Italic, Link, List, ListOrdered, Quote, RemoveFormatting, SquareCode } from "@lucide/vue";
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

type Action = { id: string; label: string; icon: typeof Bold; run: (editor: Editor, range?: Range) => void };
const block = (fn: (c: ReturnType<Editor["chain"]>) => ReturnType<Editor["chain"]>) => (editor: Editor, range?: Range) => {
  const chain = editor.chain().focus();
  if (range) chain.deleteRange(range);
  fn(chain).run();
};
const blockActions: Action[] = [
  { id: "heading", label: "Heading", icon: Heading, run: block(c => c.toggleHeading({ level: 2 })) },
  { id: "bullet-list", label: "Bullet list", icon: List, run: block(c => c.toggleBulletList()) },
  { id: "ordered-list", label: "Numbered list", icon: ListOrdered, run: block(c => c.toggleOrderedList()) },
  { id: "link", label: "Link", icon: Link, run: insertLink },
  { id: "quote", label: "Quote", icon: Quote, run: block(c => c.toggleBlockquote()) },
  { id: "code-block", label: "Code block", icon: SquareCode, run: block(c => c.toggleCodeBlock()) },
  { id: "image", label: "Image", icon: ImagePlus, run: insertImage },
];
const inlineActions: (Action & { mark?: string })[] = [
  { id: "bold", label: "Bold", icon: Bold, mark: "bold", run: block(c => c.toggleBold()) },
  { id: "italic", label: "Italic", icon: Italic, mark: "italic", run: block(c => c.toggleItalic()) },
  { id: "code", label: "Inline code", icon: Code, mark: "code", run: block(c => c.toggleCode()) },
  { id: "clear", label: "Clear formatting", icon: RemoveFormatting, run: block(c => c.unsetAllMarks()) },
  { id: "link", label: "Link", icon: Link, mark: "link", run: insertLink },
];
function runTool(action: Action) { if (richEditor.value) action.run(richEditor.value); }

// ---- Selection menu ------------------------------------------------------------------------
type BlockType = { id: string; label: string; active: (e: Editor) => boolean };
/** Checked in order: the first active one names the selection's block. */
const blockTypes: BlockType[] = [
  { id: "code-block", label: "Code block", active: e => e.isActive("codeBlock") },
  { id: "bullet-list", label: "Bullet list", active: e => e.isActive("bulletList") },
  { id: "ordered-list", label: "Numbered list", active: e => e.isActive("orderedList") },
  { id: "quote", label: "Quote", active: e => e.isActive("blockquote") },
  { id: "h1", label: "Heading 1", active: e => e.isActive("heading", { level: 1 }) },
  { id: "h2", label: "Heading 2", active: e => e.isActive("heading", { level: 2 }) },
  { id: "h3", label: "Heading 3", active: e => e.isActive("heading", { level: 3 }) },
  { id: "text", label: "Text", active: () => true },
];
const blockMenu = ["text", "h1", "h2", "h3", "bullet-list", "ordered-list", "quote", "code-block"].map(id => blockTypes.find(b => b.id === id)!);
/** Bumped on every editor transaction so the menu re-reads active marks and blocks. */
const revision = ref(0);
const bubble = ref<{ top: number; left: number } | null>(null);
const bubbleHost = ref<HTMLElement | null>(null);
const bubbleEl = ref<HTMLElement | null>(null);
const currentBlock = computed(() => {
  void revision.value;
  const e = richEditor.value;
  return e ? blockTypes.find(b => b.active(e))!.id : "text";
});
function markActive(mark?: string) {
  void revision.value;
  return !!mark && !!richEditor.value?.isActive(mark);
}
function setBlock(id: string) {
  const e = richEditor.value; if (!e) return;
  const c = e.chain().focus();
  if (e.isActive("codeBlock") && id !== "code-block") c.toggleCodeBlock();
  if (e.isActive("bulletList") && id !== "bullet-list") c.toggleBulletList();
  if (e.isActive("orderedList") && id !== "ordered-list") c.toggleOrderedList();
  if (e.isActive("blockquote") && id !== "quote") c.toggleBlockquote();
  if (id === "text") c.setParagraph();
  else if (id === "h1" || id === "h2" || id === "h3") c.setHeading({ level: Number(id[1]) as 1 | 2 | 3 });
  else if (id === "bullet-list" && !e.isActive("bulletList")) c.toggleBulletList();
  else if (id === "ordered-list" && !e.isActive("orderedList")) c.toggleOrderedList();
  else if (id === "quote" && !e.isActive("blockquote")) c.toggleBlockquote();
  else if (id === "code-block" && !e.isActive("codeBlock")) c.toggleCodeBlock();
  c.run();
}
/** Float the menu above a non-empty text selection; hide it otherwise. */
function placeBubble(editor: Editor) {
  const { from, to, empty } = editor.state.selection;
  if (empty || editor.state.selection instanceof NodeSelection || !bubbleHost.value) { bubble.value = null; return; }
  const host = bubbleHost.value.getBoundingClientRect();
  try {
    const start = editor.view.coordsAtPos(from), end = editor.view.coordsAtPos(to);
    // A selection that wraps lines starts right of where it ends; centre over the first line then.
    const right = end.top > start.top ? host.right : end.right;
    const mid = (start.left + right) / 2 - host.left;
    bubble.value = { top: Math.min(start.top, end.top) - host.top, left: mid };
  } catch {
    bubble.value = { top: 0, left: 0 };
  }
  void nextTick(clampBubble);
}
/** Keep the centred menu inside the editor column. */
function clampBubble() {
  const el = bubbleEl.value, host = bubbleHost.value;
  if (!el || !host || !bubble.value) return;
  const half = el.offsetWidth / 2;
  const left = Math.min(Math.max(bubble.value.left, half), Math.max(host.clientWidth - half, half));
  if (left !== bubble.value.left) bubble.value = { ...bubble.value, left };
}
function editorBlurred(event: FocusEvent) {
  const next = event.relatedTarget as Node | null;
  if (next && bubbleEl.value?.contains(next)) return;
  bubble.value = null;
}

const slash = shallowRef<{ items: Action[]; command: (item: Action) => void; active: number; top: number; left: number } | null>(null);
const SLASH_WIDTH = 192;
/** Just under the `/`, relative to the editor and kept inside its width. */
function slashPosition(rect: DOMRect | null | undefined) {
  const host = bubbleHost.value;
  if (!rect || !host) return { top: 0, left: 0 };
  const box = host.getBoundingClientRect();
  const left = Math.min(rect.left - box.left, host.clientWidth - SLASH_WIDTH);
  return { top: rect.bottom - box.top + 4, left: Math.max(left, 0) };
}
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
        const show = (p: SuggestionProps<Action, Action>) => {
          slash.value = { items: p.items, command: p.command, active: 0, ...slashPosition(p.clientRect?.()) };
        };
        return { onStart: show, onUpdate: show, onExit: () => { slash.value = null; }, onKeyDown: slashKey };
      },
    })];
  },
});

// ---- Reordering blocks ---------------------------------------------------------------------
/** The block a move acts on: the list item when inside one, otherwise the top-level block. */
function movableDepth($pos: ResolvedPos) {
  for (let d = $pos.depth; d > 0; d--) if ($pos.node(d).type.name === "listItem") return d;
  return 1;
}
function moveBlock(editor: Editor, dir: -1 | 1) {
  const { state } = editor;
  const { $from, from } = state.selection;
  if ($from.depth < 1) return false;
  const d = movableDepth($from);
  const parent = $from.node(d - 1), index = $from.index(d - 1), target = index + dir;
  if (target < 0 || target >= parent.childCount) return true;
  const node = $from.node(d), start = $from.before(d), sibling = parent.child(target);
  const at = dir < 0 ? start - sibling.nodeSize : start + sibling.nodeSize;
  const tr = state.tr.delete(start, start + node.nodeSize).insert(at, node);
  tr.setSelection(TextSelection.create(tr.doc, at + (from - start)));
  editor.view.dispatch(tr.scrollIntoView());
  return true;
}
const BlockMove = Extension.create({
  name: "blockMove",
  addKeyboardShortcuts() {
    return {
      "Alt-ArrowUp": () => moveBlock(this.editor as Editor, -1),
      "Alt-ArrowDown": () => moveBlock(this.editor as Editor, 1),
    };
  },
});

const HANDLE_GUTTER = 28;
const handle = ref<{ top: number; pos: number; el: HTMLElement } | null>(null);
let draggingBlock = false;
/** The top-level block, or list item, under the pointer's line. */
function blockAt(clientY: number): HTMLElement | null {
  const view = richEditor.value?.view;
  if (!view) return null;
  const within = (el: Element) => { const r = el.getBoundingClientRect(); return clientY >= r.top && clientY <= r.bottom; };
  const block = ([...view.dom.children] as HTMLElement[]).find(within);
  if (block && (block.tagName === "UL" || block.tagName === "OL")) return ([...block.children] as HTMLElement[]).find(within) ?? block;
  return block ?? null;
}
function hoverBlock(event: MouseEvent) {
  if (draggingBlock || (event.target as HTMLElement).closest?.("[data-drag-handle]")) return;
  const view = richEditor.value?.view, host = bubbleHost.value, el = blockAt(event.clientY);
  if (!view || !host || !el) { handle.value = null; return; }
  try {
    const $p = view.state.doc.resolve(view.posAtDOM(el, 0));
    const pos = $p.depth === 0 ? $p.pos : $p.before(movableDepth($p));
    const r = el.getBoundingClientRect();
    const line = Math.min(parseFloat(getComputedStyle(el).lineHeight) || 28, r.height);
    handle.value = { pos, el, top: r.top - host.getBoundingClientRect().top + line / 2 };
  } catch {
    handle.value = null;
  }
}
/** Drag the hovered block with ProseMirror's own move-on-drop. */
function dragBlock(event: DragEvent) {
  const view = richEditor.value?.view, h = handle.value;
  if (!view || !h || !event.dataTransfer) return;
  const selection = NodeSelection.create(view.state.doc, h.pos);
  view.dispatch(view.state.tr.setSelection(selection));
  event.dataTransfer.effectAllowed = "move";
  event.dataTransfer.setData("text/plain", h.el.textContent ?? "");
  event.dataTransfer.setDragImage(h.el, 0, 0);
  view.dragging = { slice: selection.content(), move: true };
  draggingBlock = true;
}
function dragBlockEnd() { draggingBlock = false; handle.value = null; }

// ---- Modes ---------------------------------------------------------------------------------
function destroyRich() { richEditor.value?.destroy(); richEditor.value = null; slash.value = null; bubble.value = null; handle.value = null; }
function destroyRaw() { rawView?.destroy(); rawView = null; }
function mountRich() {
  destroyRich();
  // The serializer drops the final newline(s); keep the file's own ending after an edit.
  const ending = /\n*$/.exec(text)![0];
  // Loading content does not fire `onUpdate`; only the writer's transactions do.
  richEditor.value = new Editor({
    extensions: [
      ...markdownExtensions(props.resolveImage),
      SlashCommands,
      BlockMove,
      Placeholder.configure({ placeholder: ({ editor }) => (editor.isEmpty ? "Start writing, or type / for commands…" : "Type / for commands…") }),
    ],
    content: text,
    contentType: "markdown",
    onUpdate: ({ editor }) => edited(editor.getMarkdown().replace(/\n+$/, "") + ending),
    onTransaction: () => { revision.value++; },
    onSelectionUpdate: ({ editor }) => placeBubble(editor as Editor),
    onBlur: ({ event }) => editorBlurred(event),
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
        placeholder("Write Markdown…"),
        EditorView.updateListener.of(update => { if (update.docChanged) edited(update.state.doc.toString()); }),
        EditorView.theme({
          "&": { fontSize: "13px", minHeight: "60vh" },
          "&.cm-focused": { outline: "none" },
          ".cm-scroller": { fontFamily: "var(--font-mono, ui-monospace, monospace)" },
        }),
      ],
    }),
  });
}
async function setMode(next: "rich" | "raw") {
  if (next === mode.value) return;
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

defineExpose({ richEditor: () => richEditor.value, mode, setMode });
</script>

<template>
  <div class="mt-6 mb-3" data-testid="body-editor" @paste.capture="transfer" @drop.capture="transfer" @dragover.capture="dragOver">
    <p v-if="unsupported" role="status" class="mb-1 text-xs text-muted-foreground" data-testid="body-unsupported">
      This body has Markdown the rich editor cannot represent (such as HTML or MDX), so it stays in Raw view.
    </p>
    <div v-if="mode === 'rich' && richEditor" ref="bubbleHost" class="relative" @mousemove="hoverBlock" @mouseleave="draggingBlock || (handle = null)">
      <div
        v-if="handle"
        data-drag-handle
        data-testid="body-drag-handle"
        draggable="true"
        title="Drag to move · Alt+↑/↓"
        class="absolute z-10 flex h-7 -translate-y-1/2 cursor-grab items-center justify-center text-muted-foreground/60 hover:text-foreground active:cursor-grabbing"
        :style="{ top: `${handle.top}px`, left: `-${HANDLE_GUTTER}px`, width: `${HANDLE_GUTTER}px` }"
        @dragstart="dragBlock"
        @dragend="dragBlockEnd"
      >
        <span class="flex size-5 items-center justify-center rounded hover:bg-accent"><GripVertical class="size-4" /></span>
      </div>
      <EditorContent :editor="richEditor" data-testid="body-rich" class="content-rich text-[15px] leading-7" />
      <div
        v-if="bubble"
        ref="bubbleEl"
        role="toolbar"
        aria-label="Formatting"
        data-testid="body-bubble"
        class="absolute z-20 flex -translate-x-1/2 -translate-y-full items-center gap-0.5 rounded-lg border bg-popover p-1 text-popover-foreground shadow-lg"
        :style="{ top: `${bubble.top - 8}px`, left: `${bubble.left}px` }"
      >
        <label class="relative flex items-center">
          <span class="sr-only">Block type</span>
          <select
            data-testid="body-block-type"
            class="h-7 cursor-pointer appearance-none rounded-md bg-transparent py-0 pr-7 pl-2 text-sm outline-none hover:bg-accent"
            :value="currentBlock"
            @change="setBlock(($event.target as HTMLSelectElement).value)"
          >
            <option v-for="b in blockMenu" :key="b.id" :value="b.id" class="bg-popover">{{ b.label }}</option>
          </select>
          <ChevronDown class="pointer-events-none absolute right-2 size-3.5 text-muted-foreground" />
        </label>
        <div class="mx-1 h-5 w-px bg-border" aria-hidden="true" />
        <button
          v-for="action in inlineActions"
          :key="action.id"
          type="button"
          :title="action.label"
          :aria-label="action.label"
          :aria-pressed="markActive(action.mark)"
          :data-testid="`body-tool-${action.id}`"
          class="flex size-7 items-center justify-center rounded-md hover:bg-accent aria-pressed:bg-accent aria-pressed:text-accent-foreground"
          @mousedown.prevent
          @click="runTool(action)"
        >
          <component :is="action.icon" class="size-4" />
        </button>
      </div>
      <ul v-if="slash && slash.items.length" class="absolute z-10 w-48 rounded-md border bg-popover p-1 text-sm shadow" :style="{ top: `${slash.top}px`, left: `${slash.left}px` }" role="listbox" data-testid="slash-menu">
        <li v-for="(item, index) in slash.items" :key="item.id" role="option" :aria-selected="index === slash.active" data-testid="slash-item" class="flex cursor-pointer items-center gap-2 rounded px-2 py-1" :class="{ 'bg-muted': index === slash.active }" @mousedown.prevent="slash.command(item)"><component :is="item.icon" class="size-4 shrink-0 text-muted-foreground" />{{ item.label }}</li>
      </ul>
    </div>
    <div v-show="mode === 'raw'" ref="rawHost" data-testid="body-raw" />
    <ImageDialog v-model:open="imageOpen" :file="imageFile" @insert="imageInserted" />
  </div>
</template>

<style scoped>
.content-rich :deep(.ProseMirror) { outline: none; min-height: 60vh; }
.content-rich :deep(h1) { font-size: 1.875rem; font-weight: 600; line-height: 1.25; }
.content-rich :deep(h2) { font-size: 1.5rem; font-weight: 600; line-height: 1.3; }
.content-rich :deep(h3) { font-size: 1.2rem; font-weight: 600; }
.content-rich :deep(::selection) { background: color-mix(in oklch, var(--primary) 30%, transparent); }
.content-rich :deep(ul) { list-style: disc; padding-left: 1.5rem; }
.content-rich :deep(ol) { list-style: decimal; padding-left: 1.5rem; }
.content-rich :deep(blockquote) { border-left: 3px solid var(--border); padding-left: 0.75rem; }
.content-rich :deep(pre) { background: var(--muted); border-radius: 0.375rem; padding: 0.5rem; font-family: var(--font-mono, ui-monospace, monospace); }
.content-rich :deep(code) { font-family: var(--font-mono, ui-monospace, monospace); }
.content-rich :deep(a) { text-decoration: underline; }
.content-rich :deep(img) { max-width: 100%; }
.content-rich :deep(.is-empty)::before { content: attr(data-placeholder); float: left; height: 0; pointer-events: none; color: var(--muted-foreground); }
.content-rich :deep(p), .content-rich :deep(blockquote), .content-rich :deep(pre) { margin: 0.5rem 0; }
.content-rich :deep(h1), .content-rich :deep(h2), .content-rich :deep(h3) { margin: 1.25rem 0 0.5rem; }
</style>
