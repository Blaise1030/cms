<script setup lang="ts">
import { computed, inject, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { Extension, getMarkRange, type Editor } from "@tiptap/core";
import { EditorState } from "@codemirror/state";
import type { ResolvedPos } from "@tiptap/pm/model";
import { NodeSelection, Plugin, TextSelection } from "@tiptap/pm/state";
import { CellSelection } from "@tiptap/pm/tables";
import { EditorView, placeholder } from "@codemirror/view";
import { markdown as markdownLanguage } from "@codemirror/lang-markdown";
import type { EditorCustomHandlers, EditorSuggestionMenuItem, EditorToolbarItem } from "@nuxt/ui";
import UEditor from "@nuxt/ui/components/Editor.vue";
import UEditorDragHandle from "@nuxt/ui/components/EditorDragHandle.vue";
import UEditorSuggestionMenu from "@nuxt/ui/components/EditorSuggestionMenu.vue";
import UEditorToolbar from "@nuxt/ui/components/EditorToolbar.vue";
import USelect from "@nuxt/ui/components/Select.vue";
import { bodyExtensions, canRoundTrip, STARTER_KIT } from "./markdown";
import ImageDialog, { IMAGE_REGISTRY, markdownImage, type PendingImage } from "./ImageDialog.vue";
import LinkPopover from "./LinkPopover.vue";
import { keySound, loadPack, playSample, typesText, useTypingSound } from "@/lib/typingSound";

/**
 * Body editor for Markdown/MDX entries. `body` is the draft body and `original` the loaded one.
 * Rich mode (Nuxt UI `UEditor`, Tiptap underneath) is offered only when `rich` is set for a
 * Markdown entry and the text can be represented by the configured extensions; everything else is
 * edited raw (CodeMirror).
 * The text is kept byte-for-byte until the writer edits it: opening or toggling never reserializes.
 */
/** `resolveImage` maps an image link to the URL the rich editor displays; the text keeps the link. */
const props = defineProps<{ body: string; original: string; kind: "markdown" | "mdx"; rich: boolean; resolveImage?: (src: string) => string }>();
const emit = defineEmits<{
  change: [value: { body: string; dirty: boolean }];
  /** "Ask AI" over a selection: the selected text, for the parent to hand to the agent. */
  ask: [text: string];
}>();

const richCapable = computed(() => props.rich && props.kind === "markdown");
/** The current body text; only replaced by a real edit or an external reset. */
let text = props.body;
const mode = ref<"rich" | "raw">("raw");
/** Shown when the text holds Markdown the rich editor cannot represent. */
const unsupported = ref(false);
const editorRef = shallowRef<{ editor?: Editor } | null>(null);
const richEditor = computed<Editor | null>(() => (mode.value === "rich" && editorRef.value?.editor) || null);
const host = ref<HTMLElement | null>(null);
const slashHost = ref<HTMLElement | null>(null);
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
function insertRich(editor: Editor, image: PendingImage) {
  // The Markdown parser keeps `\\` in alt text as written, so store backslashes the way it would
  // read them back from `escapeAlt` output.
  editor.chain().focus().setImage({ src: image.previewUrl, alt: image.alt.replace(/\\/g, "\\\\") }).run();
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
function insertImage(editor: Editor) {
  requestImage(null, image => insertRich(editor, image));
}
/** The selection toolbar's Tiptap plugin key; a `hide` meta under it closes the toolbar. */
const BUBBLE_KEY = "bodyBubble";
type LinkCard = { top: number; left: number; href?: string; editing: boolean; askText: boolean };
const link = ref<LinkCard | null>(null);
const linkCard = ref<InstanceType<typeof LinkPopover> | null>(null);
const LINK_WIDTH = 320;
/** Under the start of the link (or selection), kept inside the editor column. */
function linkPosition(editor: Editor) {
  const box = host.value;
  if (!box) return { top: 0, left: 0 };
  const { state } = editor;
  const type = state.schema.marks.link!;
  const from = getMarkRange(state.selection.$from, type)?.from ?? state.selection.from;
  const rect = box.getBoundingClientRect();
  try {
    const at = editor.view.coordsAtPos(from);
    const left = Math.min(at.left - rect.left, box.clientWidth - LINK_WIDTH);
    return { top: at.bottom - rect.top + 6, left: Math.max(left, 0) };
  } catch {
    return { top: 0, left: 0 };
  }
}
/** Show the link under the cursor; an open edit stays until applied or cancelled. */
function syncLink(editor: Editor) {
  if (link.value?.editing) return;
  link.value = editor.isActive("link")
    ? { href: editor.getAttributes("link").href as string | undefined, editing: false, askText: false, ...linkPosition(editor) }
    : null;
}
/** Edit the link under the cursor, link the selection, or add a new link at the cursor. */
function insertLink(editor: Editor) {
  const href = editor.isActive("link") ? (editor.getAttributes("link").href as string | undefined) : undefined;
  link.value = { href, editing: true, askText: !href && editor.state.selection.empty, ...linkPosition(editor) };
  editor.view.dispatch(editor.state.tr.setMeta(BUBBLE_KEY, "hide"));
}
function editLink() { if (link.value) link.value = { ...link.value, editing: true, askText: false }; }
function closeLink() {
  link.value = null;
  richEditor.value?.commands.focus();
}
function applyLink({ href, text }: { href: string; text: string }) {
  const editor = richEditor.value, card = link.value;
  if (!editor || !card) return;
  link.value = null;
  const chain = editor.chain().focus();
  if (card.askText) chain.insertContent({ type: "text", text: text || href, marks: [{ type: "link", attrs: { href } }] }).run();
  else chain.extendMarkRange("link").setLink({ href }).run();
  syncLink(editor);
}
function removeLink() {
  const editor = richEditor.value; if (!editor) return;
  link.value = null;
  editor.chain().focus().extendMarkRange("link").unsetLink().run();
}
/** Focus leaving the card for anywhere but the editor drops an unfinished edit. */
function linkFocusOut(event: FocusEvent) {
  const next = event.relatedTarget as Node | null;
  if (next && (linkCard.value?.root?.contains(next) || richEditor.value?.view.dom.contains(next))) return;
  link.value = null;
}

type Action = { id: string; label: string; icon: string; run: (editor: Editor) => void };
const block = (fn: (c: ReturnType<Editor["chain"]>) => ReturnType<Editor["chain"]>) => (editor: Editor) => {
  fn(editor.chain().focus()).run();
};
const blockActions: Action[] = [
  { id: "heading", label: "Heading", icon: "i-hugeicons-heading", run: block(c => c.toggleHeading({ level: 2 })) },
  { id: "bullet-list", label: "Bullet list", icon: "i-hugeicons-left-to-right-list-bullet", run: block(c => c.toggleBulletList()) },
  { id: "ordered-list", label: "Numbered list", icon: "i-hugeicons-left-to-right-list-number", run: block(c => c.toggleOrderedList()) },
  { id: "link", label: "Link", icon: "i-hugeicons-link-01", run: insertLink },
  { id: "quote", label: "Quote", icon: "i-hugeicons-quote-down", run: block(c => c.toggleBlockquote()) },
  { id: "code-block", label: "Code block", icon: "i-hugeicons-code-square", run: block(c => c.toggleCodeBlock()) },
  { id: "image", label: "Image", icon: "i-hugeicons-image-add-01", run: insertImage },
  // GFM tables need a header row, so a new table always starts with one.
  { id: "table", label: "Table", icon: "i-hugeicons-table", run: block(c => c.insertTable({ rows: 3, cols: 3, withHeaderRow: true })) },
];
const inlineActions: (Action & { mark?: string })[] = [
  { id: "bold", label: "Bold", icon: "i-hugeicons-text-bold", mark: "bold", run: block(c => c.toggleBold()) },
  { id: "italic", label: "Italic", icon: "i-hugeicons-text-italic", mark: "italic", run: block(c => c.toggleItalic()) },
  { id: "code", label: "Inline code", icon: "i-hugeicons-source-code", mark: "code", run: block(c => c.toggleCode()) },
  { id: "clear", label: "Clear formatting", icon: "i-hugeicons-remove-formatting", run: block(c => c.unsetAllMarks()) },
  { id: "link", label: "Link", icon: "i-hugeicons-link-01", mark: "link", run: insertLink },
];
function runTool(action: Action) { if (richEditor.value) action.run(richEditor.value); }

/**
 * Slash commands. `UEditorSuggestionMenu` deletes the typed `/query`, then runs the item's handler.
 * Its filter ranks prefix matches first; matching on `search` (the label behind a separator) makes
 * every hit an equal "contains" match, so filtered commands keep the list's own order.
 */
const handlers = {
  slash: {
    canExecute: () => true,
    isActive: () => false,
    execute: (editor: Editor, item: { id: string }) => ({ run: () => { blockActions.find(a => a.id === item.id)?.run(editor); return true; } }),
  },
} satisfies EditorCustomHandlers;
const slashItems: EditorSuggestionMenuItem<typeof handlers>[] = blockActions.map(a => ({ kind: "slash", id: a.id, label: a.label, icon: a.icon, search: `·${a.label}` }));

// ---- Selection toolbar ---------------------------------------------------------------------
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
const blockMenu = ["text", "h1", "h2", "h3", "bullet-list", "ordered-list", "quote", "code-block"]
  .map(id => blockTypes.find(b => b.id === id)!)
  .map(b => ({ label: b.label, value: b.id }));
/** Bumped on every editor transaction so the toolbar re-reads active marks and blocks. */
const revision = ref(0);
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
/** Toolbar buttons keep the editor's focus and selection. */
const keepFocus = (event: MouseEvent) => event.preventDefault();
const toolbarItems = computed<EditorToolbarItem[][]>(() => [
  [{ slot: "blockType" }],
  inlineActions.map(action => ({
    "icon": action.icon,
    "square": true,
    "title": action.label,
    "aria-label": action.label,
    "aria-pressed": markActive(action.mark),
    "active": markActive(action.mark),
    "data-testid": `body-tool-${action.id}`,
    "onMousedown": keepFocus,
    "onClick": () => runTool(action),
  }) as EditorToolbarItem),
  [{
    "icon": "i-hugeicons-ai-magic",
    "label": "Ask AI",
    "title": "Ask the assistant about this selection",
    "data-testid": "body-tool-ask-ai",
    "onMousedown": keepFocus,
    "onClick": askAI,
  } as EditorToolbarItem],
]);
/** Send the selected text to the assistant; the parent adds which file it is in. */
function askAI() {
  const e = richEditor.value;
  if (!e) return;
  const { from, to } = e.state.selection;
  const text = e.state.doc.textBetween(from, to, "\n").trim();
  if (text) emit("ask", text);
}
/** Over a non-empty text selection, except while a link is being edited (cell selections get the table toolbar). */
function showToolbar({ state }: { state: Editor["state"] }) {
  const { selection } = state;
  return !selection.empty && !(selection instanceof NodeSelection) && !(selection instanceof CellSelection) && !link.value?.editing;
}

// ---- Table toolbar -------------------------------------------------------------------------
const TABLE_BUBBLE_KEY = "tableBubble";
/**
 * Rows and columns only: merged cells and cell attributes have no GFM Markdown form, so the
 * toolbar does not offer them.
 */
const tableActions: Action[] = [
  { id: "row-above", label: "Insert row above", icon: "i-hugeicons-insert-row-up", run: block(c => c.addRowBefore()) },
  { id: "row-below", label: "Insert row below", icon: "i-hugeicons-insert-row-down", run: block(c => c.addRowAfter()) },
  { id: "column-left", label: "Insert column left", icon: "i-hugeicons-insert-column-left", run: block(c => c.addColumnBefore()) },
  { id: "column-right", label: "Insert column right", icon: "i-hugeicons-insert-column-right", run: block(c => c.addColumnAfter()) },
  { id: "delete-row", label: "Delete row", icon: "i-hugeicons-delete-row", run: block(c => c.deleteRow()) },
  { id: "delete-column", label: "Delete column", icon: "i-hugeicons-delete-column", run: block(c => c.deleteColumn()) },
  { id: "delete-table", label: "Delete table", icon: "i-hugeicons-delete-02", run: block(c => c.deleteTable()) },
];
const tableToolbarItems = computed<EditorToolbarItem[][]>(() => {
  const item = (action: Action) => ({
    "icon": action.icon,
    "square": true,
    "title": action.label,
    "aria-label": action.label,
    "data-testid": `table-tool-${action.id}`,
    "onMousedown": keepFocus,
    "onClick": () => runTool(action),
  }) as EditorToolbarItem;
  return [tableActions.slice(0, 4).map(item), tableActions.slice(4, 6).map(item), [item(tableActions[6]!)]];
});
/** With the cursor (or a cell selection) in a table; a text selection gets the formatting toolbar. */
function showTableToolbar({ editor, state }: { editor: Editor; state: Editor["state"] }) {
  const { selection } = state;
  return editor.isActive("table") && (selection.empty || selection instanceof CellSelection) && !link.value?.editing;
}
function editorBlurred(event: FocusEvent) {
  const next = event.relatedTarget as Node | null;
  if (next && linkCard.value?.root?.contains(next)) return;
  if (!link.value?.editing) link.value = null;
}

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
/** The drag grip offers top-level blocks and list items, the same blocks Alt+↑/↓ moves. */
const dragTargets = {
  rules: [{
    id: "topLevelOrListItem",
    evaluate: ({ node, depth }: { node: { type: { name: string } }; depth: number }) =>
      depth === 1 || node.type.name === "listItem" || node.type.name === "taskItem" ? 0 : 1000,
  }],
};

const typingSound = useTypingSound();
/** Release samples for keys still held down, by `KeyboardEvent.code`. */
const heldKeys = new Map<string, AudioBuffer>();
const TypingSound = Extension.create({
  name: "typingSound",
  addProseMirrorPlugins() {
    return [new Plugin({
      props: {
        handleKeyDown: (_view, event) => {
          const pack = typingSound.setting.value;
          if (pack === "off" || !typesText(event)) return false;
          const release = keySound(pack, event.key);
          if (release) heldKeys.set(event.code, release);
          return false;
        },
        handleDOMEvents: {
          keyup: (_view, event) => {
            const release = heldKeys.get(event.code);
            heldKeys.delete(event.code);
            if (release) playSample(release);
            return false;
          },
        },
      },
    })];
  },
});

// ---- Modes ---------------------------------------------------------------------------------
/** What the current rich editor was opened with; fixed for its lifetime so `UEditor` never re-sets it. */
const richSource = ref("");
/** Remounts `UEditor` for each rich session. */
const richKey = ref(0);
/** The serializer drops the final newline(s); keep the file's own ending after an edit. */
let ending = "";
const richExtensions = computed(() => [...bodyExtensions(props.resolveImage), BlockMove, TypingSound]);
const richPlaceholder = {
  placeholder: ({ editor }: { editor: Editor }) => (editor.isEmpty ? "Start writing, or type / for commands…" : "Type / for commands…"),
  showOnlyWhenEditable: true,
};
/** `UEditor` emits only for transactions that change the document; loading content does not. */
function richChanged(markdown: unknown) {
  edited(String(markdown).replace(/\n+$/, "") + ending);
}
function richBlurred({ event }: { event: FocusEvent }) { editorBlurred(event); }
function richSelected({ editor }: { editor: Editor }) { syncLink(editor); }
function richTransaction() { revision.value++; }
function destroyRich() { link.value = null; }
function destroyRaw() { rawView?.destroy(); rawView = null; }
function mountRich() {
  destroyRich();
  // Fetch the pack now so the first keystroke isn't silent.
  if (typingSound.setting.value !== "off") loadPack(typingSound.setting.value).catch(() => {});
  ending = /\n*$/.exec(text)![0];
  richSource.value = text;
  richKey.value++;
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
    destroyRaw(); mountRich(); mode.value = "rich";
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
  if (rich) mountRich();
  mode.value = rich ? "rich" : "raw";
  await nextTick();
  if (!rich) mountRaw();
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
    <p v-if="unsupported" role="status" class="text-muted mb-1 text-xs" data-testid="body-unsupported">
      This body has Markdown the rich editor cannot represent (such as HTML or MDX), so it stays in Raw view.
    </p>
    <div v-if="mode === 'rich'" ref="host" class="relative" data-testid="body-rich">
      <UEditor
        :key="richKey"
        ref="editorRef"
        v-slot="{ editor }"
        :model-value="richSource"
        content-type="markdown"
        :starter-kit="STARTER_KIT"
        :image="false"
        :mention="false"
        :extensions="richExtensions"
        :placeholder="richPlaceholder"
        :handlers="handlers"
        :on-transaction="richTransaction"
        :on-selection-update="richSelected"
        :on-blur="richBlurred"
        class="content-rich text-base leading-7"
        @update:model-value="richChanged"
      >
        <UEditorToolbar
          :editor="editor"
          layout="bubble"
          :plugin-key="BUBBLE_KEY"
          :update-delay="0"
          :should-show="showToolbar"
          :items="toolbarItems"
          active-color="neutral"
          aria-label="Formatting"
          data-testid="body-bubble"
          role="group"
          class="text-default shadow-lg"
          :ui="{ root: 'z-20' }"
        >
          <template #blockType>
            <USelect
              :model-value="currentBlock"
              :items="blockMenu"
              color="neutral"
              variant="ghost"
              size="sm"
              aria-label="Block type"
              data-testid="body-block-type"
              class="min-w-32"
              @update:model-value="setBlock(String($event))"
            />
          </template>
        </UEditorToolbar>
        <UEditorToolbar
          :editor="editor"
          layout="bubble"
          :plugin-key="TABLE_BUBBLE_KEY"
          :update-delay="0"
          :should-show="showTableToolbar"
          :items="tableToolbarItems"
          active-color="neutral"
          aria-label="Table"
          data-testid="table-bubble"
          role="group"
          class="text-default shadow-lg"
          :ui="{ root: 'z-20' }"
        />
        <UEditorSuggestionMenu
          :editor="editor"
          :items="slashItems"
          :filter-fields="['search']"
          plugin-key="slashCommands"
          :append-to="() => slashHost!"
        />
        <UEditorDragHandle
          :editor="editor"
          :nested="dragTargets"
          icon="i-hugeicons-drag-drop-vertical"
          title="Drag to move · Alt+↑/↓"
          data-testid="body-drag-handle"
          class="text-dimmed hover:text-highlighted"
          :ui="{ root: 'flex' }"
        />
      </UEditor>
      <!-- Where the slash menu opens (Nuxt UI renders it as a listbox), kept out of the prose styles. -->
      <div ref="slashHost" data-testid="slash-menu" />
      <LinkPopover
        v-if="link"
        ref="linkCard"
        class="absolute z-20"
        :style="{ top: `${link.top}px`, left: `${link.left}px` }"
        :href="link.href"
        :editing="link.editing"
        :ask-text="link.askText"
        @apply="applyLink"
        @remove="removeLink"
        @edit="editLink"
        @cancel="closeLink"
        @focusout="linkFocusOut"
      />
    </div>
    <div v-show="mode === 'raw'" ref="rawHost" data-testid="body-raw" />
    <ImageDialog v-model:open="imageOpen" :file="imageFile" @insert="imageInserted" />
  </div>
</template>

<style scoped>
/*
 * The body keeps its own prose look rather than UEditor's: these rules restate it and neutralise
 * the theme's spacing, link, inline-code, list and heading styles it would otherwise pick up.
 */
.content-rich :deep(.ProseMirror) { outline: none; min-height: 60vh; padding: 0; }
.content-rich :deep(:where(.ProseMirror > *)) { margin: 0; }
.content-rich :deep(h1) { font-size: 1.875rem; font-weight: 600; line-height: 1.25; }
.content-rich :deep(h2) { font-size: 1.5rem; font-weight: 600; line-height: 1.3; }
.content-rich :deep(h3) { font-size: 1.2rem; font-weight: 600; line-height: inherit; }
.content-rich :deep(:is(h1, h2, h3, h4, h5, h6)) { color: inherit; }
.content-rich :deep(:is(h4, h5, h6)) { font-size: inherit; font-weight: inherit; line-height: inherit; }
.content-rich :deep(::selection) { background: color-mix(in oklch, var(--ui-primary) 30%, transparent); }
.content-rich :deep(ul) { list-style: disc; padding-left: 1.5rem; }
.content-rich :deep(ol) { list-style: decimal; padding-left: 1.5rem; }
.content-rich :deep(li) { margin: 0; padding-left: 0; }
.content-rich :deep(li::marker) { color: inherit; }
.content-rich :deep(blockquote) { border-left: 3px solid var(--ui-border); padding-left: 0.75rem; font-style: normal; }
.content-rich :deep(pre) { background: var(--ui-bg-muted); border: 0; border-radius: 0.375rem; padding: 0.5rem; font-family: var(--font-mono, ui-monospace, monospace); font-size: inherit; line-height: inherit; }
.content-rich :deep(code) { font-family: var(--font-mono, ui-monospace, monospace); display: inline; padding: 0; border: 0; border-radius: 0; background: none; color: inherit; font-size: inherit; font-weight: inherit; }
.content-rich :deep(a) { text-decoration: underline; color: inherit; border-bottom: 0; font-weight: inherit; }
.content-rich :deep(img) { max-width: 100%; display: inline; border-radius: 0; }
.content-rich :deep([data-type="horizontalRule"]) { margin: 0; padding: 0; }
.content-rich :deep(table) { border-collapse: collapse; width: 100%; margin: 0.75rem 0; font-size: 0.875rem; line-height: 1.5; }
.content-rich :deep(th), .content-rich :deep(td) { border: 1px solid var(--ui-border); padding: 0.375rem 0.625rem; vertical-align: top; }
.content-rich :deep(th) { background: var(--ui-bg-muted); font-weight: 600; text-align: left; }
.content-rich :deep(td p), .content-rich :deep(th p) { margin: 0; }
.content-rich :deep(.selectedCell) { background: color-mix(in oklch, var(--ui-primary) 15%, transparent); }
.content-rich :deep(.is-empty)::before { content: attr(data-placeholder); float: left; height: 0; pointer-events: none; color: var(--ui-text-muted); }
.content-rich :deep(p), .content-rich :deep(blockquote), .content-rich :deep(pre) { margin: 0.5rem 0; }
.content-rich :deep(h1), .content-rich :deep(h2), .content-rich :deep(h3) { margin: 1.25rem 0 0.5rem; }
</style>
