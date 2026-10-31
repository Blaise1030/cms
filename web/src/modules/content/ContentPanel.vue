<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, provide, ref, watch } from "vue";
import { useElementBounding, useWindowSize } from "@vueuse/core";
import { ChevronLeft, FileText, PanelLeft, PanelRight, Pencil, Search, Trash2 } from "@lucide/vue";
import { toast } from "vue-sonner";
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRouter } from "vue-router";
import { contentPath } from "@/router";
import { Button } from "@/components/ui/button";
import { Sidebar, SidebarContent, SidebarGroup, SidebarGroupContent, SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable";
import { AlertDialog, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useDaemon } from "@/core/daemon";
import NewChangeDialog from "@/modules/changes/NewChangeDialog.vue";
import { useChangesStore } from "@/modules/changes/store";
import type { Change, ContentCollection } from "@/core/types";
import EntryList from "./EntryList.vue";
import { entryActions, type EntryAction } from "./entryList";
import { humanize } from "./labels";
import FrontmatterForm from "./FrontmatterForm.vue";
import { diffDraft, sameValue } from "./draft";
import { useContentHistoryStore } from "./history/store";
import VersionDiff from "./history/VersionDiff.vue";
import VersionsList from "./history/VersionsList.vue";
import { startingFields } from "./create";
import { entryAssetPath } from "./markdown";
import { useContentStore } from "./store";
import { IMAGE_REGISTRY } from "./ImageDialog.vue";
import { REFERENCE_ENTRIES } from "./references";
// Tiptap and CodeMirror load with the first Markdown/MDX entry, not with the app shell.
const MarkdownBodyEditor = defineAsyncComponent(() => import("./MarkdownBodyEditor.vue"));
const props = defineProps<{projectId: string; collection?: string; entry?: string}>();
const store = useContentStore();
const router = useRouter();
const changes = useChangesStore();
const daemon = useDaemon();
// Images inserted by the form or body editor stay pending in the draft until Save.
provide(IMAGE_REGISTRY, { add: image => store.addImage(image), resolve: src => resolveImage(src) });
provide(REFERENCE_ENTRIES, { entries: collection => store.referenceEntries[collection], load: collection => void store.loadReferenceEntries(collection) });
watch(() => props.projectId, id => void store.open(id), {immediate:true});
onBeforeRouteLeave(() => store.canLeave());
onBeforeRouteUpdate(() => store.canLeave());
const showNewChange = ref(false);
async function save() {
  try {
    await store.saveDraft(store.creating ? currentBranch() : undefined);
    showInUrl();
  } catch { /* recorded on the store; the draft stays */ }
}
/** Put what the store shows into the URL, after an action (create, rename, delete) changed it. */
function showInUrl() {
  const e = store.entry;
  void router.replace(contentPath(props.projectId, e?.collection ?? store.selectedCollection, e?.id));
}
/** A singleton's entry is update-only: it cannot be renamed or deleted. */
function removable(collection: string) {
  return !store.collections.find(x => x.name === collection)?.singleton;
}
const canRemove = computed(() => !!store.entry && !store.creating && !!store.descriptor && !store.readOnlyReason && removable(store.entry.collection));
const lifecycleOpen = computed({
  // Hidden while waiting for Create Change so the form's prompt is reachable.
  get: () => !!store.lifecycle && !store.needsChange,
  set: open => { if (!open) store.cancelLifecycle(); },
});
const canConfirm = computed(() => {
  const l = store.lifecycle;
  if (!l?.impact || l.busy) return false;
  return l.action === "rename" ? !!l.slug.trim() && l.slug.trim() !== l.id : !l.impact.references.length;
});
async function confirmLifecycle() { try { await store.confirmLifecycle(); showInUrl(); } catch { /* recorded on the store; the draft stays */ } }
/** The branch the project is on, as the Changes picker knows it. */
const currentBranch = () => changes.active?.branch ?? "main";
const collectionsPanel = ref<{ collapse: () => void; expand: () => void } | null>(null);
/** The sidebar's filter; it matches the name the writer reads, not the raw collection id. */
const collectionQuery = ref("");
const filteredCollections = computed(() => {
  const q = collectionQuery.value.trim().toLowerCase();
  return q ? validCollections.value.filter(c => humanize(c.name).toLowerCase().includes(q)) : validCollections.value;
});
const sidebarOpen = ref(true);
const fieldsOpen = ref(true);
function toggleSidebar() {
  if (sidebarOpen.value) collectionsPanel.value?.collapse(); else collectionsPanel.value?.expand();
}
/** Markdown and MDX entries edit the body in the page and the fields beside it. */
const hasFieldsPanel = computed(() => !!store.descriptor && !!store.entry && store.entry.kind !== "json");
const history = useContentHistoryStore();
const tab = ref<"fields" | "history">("fields");
/** JSON entries edit their fields in the page, so their side panel only has History. */
const sideTab = computed(() => (hasFieldsPanel.value ? tab.value : "history"));
const entryKey = computed(() => (store.entry ? `${store.entry.collection}/${store.entry.id}` : null));
/** The URL slug is the entry's file name; it renames the document when it changes. */
const slugDraft = ref("");
const slugDirty = computed(() => !!store.entry && slugDraft.value.trim() !== store.entry.id);
/** A collection that declares its own `slug` field uses that as the address; do not show two. */
const schemaHasSlug = computed(() => !!store.descriptor?.properties?.slug);
const canRenameEntry = computed(() => !!store.entry && removable(store.entry.collection) && !schemaHasSlug.value);
watch(entryKey, () => { slugDraft.value = store.entry?.id ?? ""; }, { immediate: true });
/** Save first (a rename is refused while a draft is open), then confirm the move with its impact. */
async function commitSlug() {
  const entry = store.entry;
  const next = slugDraft.value.trim();
  if (!entry || !next || next === entry.id) { slugDraft.value = entry?.id ?? ""; return; }
  try { if (store.dirty) await store.saveDraft(); } catch { return; }
  store.startLifecycle("rename");
  if (store.lifecycle) store.lifecycle.slug = next;
}
// History loads the first time its tab shows an entry; another entry drops the open version.
watch(() => [entryKey.value, sideTab.value] as const, ([key, side], old) => {
  if (key !== old?.[0]) history.close();
  const e = store.entry;
  if (e && !store.creating && side === "history" && history.key !== key) void history.open(props.projectId, e.collection, e.id);
}, { immediate: true });
// Get latest and branch switches move the file without a checkpoint event.
watch([entryKey, () => store.entry?.branch, () => store.entry?.hash], ([key], [oldKey]) => {
  if (key === oldKey && history.key === key) void history.reload();
});
onBeforeUnmount(() => history.close());
const versionDirty = computed(() => !!store.entry && diffDraft(store.entry, store.draftFields, store.draftBody, store.descriptor).changed);
const restoreReason = computed(() => {
  if (store.entry?.branch === "main") return "Start a change to restore this version.";
  if (history.version && !history.version.parseable) return "This version can't be restored because it couldn't be parsed.";
  return undefined;
});
const confirmRestore = ref(false);
function restore() { if (versionDirty.value) confirmRestore.value = true; else applyRestore(); }
function applyRestore() {
  confirmRestore.value = false;
  if (!history.version) return;
  store.restoreVersion(history.version);
  void history.select(null);
}
const wordCount = computed(() => {
  if (!hasFieldsPanel.value) return null;
  const text = store.draftBody.trim();
  return text ? text.split(/\s+/).length : 0;
});
/** The open entry decides the surface; the URL is the only thing that opens one. */
const editing = computed(() => !!props.entry || store.creating);
/** The collection the URL names, when the sidebar is willing to show it. */
const activeCollection = computed(() => validCollections.value.find(c => c.name === props.collection) ?? null);
const singletonOf = (name: string) => store.collections.find(c => c.name === name)?.singleton ?? null;
/** A singleton's back link returns to the collection index; a list's returns to its own list. */
const backTo = computed(() => contentPath(props.projectId, props.collection && singletonOf(props.collection) ? undefined : props.collection));
const backLabel = computed(() => props.collection && !singletonOf(props.collection) ? `Back to all ${props.collection}` : "Back to all content");
function backToList() {
  if (store.creating) { store.discardCreate(); return; }
  void router.push(backTo.value);
}
/** Unsupported collections are left out of the tree and reported once in a toast; ones waiting for the preview stay listed. */
const unsupported = (c: ContentCollection) => c.supportedForEditing === false;
const validCollections = computed(() => store.collections.filter(c => !unsupported(c)));
let reportedInvalid = "";
watch(() => store.collections, collections => {
  const invalid = collections.filter(unsupported);
  const key = `${props.projectId}:${invalid.map(c => c.name).join(",")}`;
  if (invalid.length && key !== reportedInvalid) {
    toast.warning(`${invalid.length === 1 ? "1 collection is" : `${invalid.length} collections are`} hidden`, {
      description: `${invalid.map(c => c.name).join(", ")}: ${[...new Set(invalid.map(c => c.reason))].join(" ")}`,
    });
  }
  reportedInvalid = key;
});
/** A deep link to a singleton collection has no list to show: send it to its one entry. */
watch(validCollections, collections => {
  const name = props.collection;
  if (!name || props.entry) return;
  const singleton = collections.find(c => c.name === name)?.singleton;
  if (singleton) void router.replace(contentPath(props.projectId, name, singleton));
}, { immediate: true });
function openEntry(collection: string, id: string) {
  void router.push(contentPath(props.projectId, collection, id));
}
/** Opening a collection skips the one-row list a singleton would have. */
function openCollection(c: ContentCollection) {
  void router.push(contentPath(props.projectId, c.name, c.singleton ?? undefined));
}
/** The row menu: the entry opens, shares, renames or deletes exactly as it did in the tree. */
function rowActions(id: string): EntryAction[] {
  const collection = props.collection ?? "";
  const path = store.entries.find(e => e.id === id)?.path;
  return entryActions({
    singleton: !!singletonOf(collection),
    path,
    dirty: store.dirty,
    handlers: {
      open: () => openEntry(collection, id),
      copyLink: () => void copy(new URL(contentPath(props.projectId, collection, id), location.origin).href, "Link"),
      copyPath: () => { if (path) void copy(path, "File path"); },
      rename: () => void lifecycleFor("rename", collection, id),
      remove: () => void lifecycleFor("delete", collection, id),
    },
  });
}
/** The URL names the open collection and entry; without one, the last collection's list comes back. */
async function follow(collection?: string, id?: string) {
  if (!collection) {
    const remembered = store.selectedCollection;
    if (store.projectId === props.projectId && remembered && !singletonOf(remembered)) void router.replace(contentPath(props.projectId, remembered));
    return;
  }
  if (!id && singletonOf(collection)) { void router.replace(contentPath(props.projectId, collection, singletonOf(collection)!)); return; }
  if (store.selectedCollection !== collection || (!id && store.entry)) {
    await store.selectCollection(collection);
    if (store.selectedCollection !== collection) return;
  }
  if (id && store.entry?.id !== id) await store.selectEntry(id);
}
watch(() => [props.collection, props.entry, props.projectId] as const, ([collection, id]) => void follow(collection, id), { immediate: true });
/** Opens the entry, then starts its rename or delete; leaving a draft is confirmed first. */
async function lifecycleFor(action: "rename" | "delete", collection: string, id: string) {
  await router.push(contentPath(props.projectId, collection, id));
  await follow(collection, id);
  if (store.entry?.collection === collection && store.entry.id === id) void store.startLifecycle(action);
}
async function copy(text: string, what: string) {
  try { await navigator.clipboard.writeText(text); toast.success(`${what} copied`); }
  catch { toast.error(`Could not copy the ${what.toLowerCase()}`); }
}
/** Create is offered for multi-entry collections; this is why it is disabled, or null when it is not. */
function createBlocked(collection: ContentCollection): string | null {
  if (collection.singleton) return null;
  if (!collection.editable || !collection.fields) return collection.reason ?? "This collection is read only";
  const start = startingFields(collection.fields, "");
  return "reason" in start ? start.reason : null;
}
/** A new entry opens as an unsaved draft; Save creates the file. */
function newEntry(name: string) { store.startCreate(name); }
/** Entry-relative image links shown in the rich editor are read from the project's files. */
function resolveImage(src: string): string {
  const e = store.entry; const path = e ? entryAssetPath(e.path, src) : null;
  return path ? daemon.api.rawFileUrl(props.projectId, path) : src;
}
async function resume(change: Change) { try { await store.resumeSave(change.branch); showInUrl(); } catch { /* recorded on the store */ } }
const differences = computed(() => {
  const now = store.current; if (!now) return [];
  const mine = store.draftFields, theirs = now.fields ?? {};
  const rows = [...new Set([...Object.keys(mine), ...Object.keys(theirs)])]
    .filter(k => !sameValue(mine[k], theirs[k])).map(k => ({ name: k, draft: mine[k], current: theirs[k] }));
  if (now.kind !== "json" && store.draftBody !== now.body) rows.push({ name: "body", draft: store.draftBody, current: now.body });
  return rows;
});
const show = (v: unknown) => v === undefined ? "(not set)" : typeof v === "string" ? v : JSON.stringify(v);
// The editor is a CSS container, so `fixed` would still place the bar against it, not the screen.
const editorForm = ref<HTMLElement | null>(null);
const { left: editorLeft, width: editorWidth } = useElementBounding(editorForm);
const { width: screenWidth } = useWindowSize();
/** The unsaved-changes bar's centre within the editor: the screen's centre, kept inside the editor. */
const unsavedCentre = computed(() => {
  const half = Math.min(448, editorWidth.value - 32) / 2;
  const centre = screenWidth.value / 2 - editorLeft.value;
  return Math.max(half + 16, Math.min(centre, editorWidth.value - half - 16));
});
function beforeUnload(event: BeforeUnloadEvent) { if(store.dirty) { event.preventDefault(); event.returnValue = ""; } }
onMounted(() => window.addEventListener("beforeunload", beforeUnload));
// File events stop refreshing Content while the tab is closed; reopening it refreshes once.
onBeforeUnmount(() => { window.removeEventListener("beforeunload", beforeUnload); store.suspend(); });
</script>
<template>
  <div class="h-full min-h-0" data-testid="content-panel">
    <SidebarProvider class="h-full min-h-0 overflow-hidden" :shortcut="false">
      <ResizablePanelGroup direction="horizontal" auto-save-id="content-sidebar" class="h-full min-h-0">
      <ResizablePanel v-if="!editing" id="content-collections" ref="collectionsPanel" :default-size="16" :min-size="10" :max-size="30" collapsible :collapsed-size="0" @collapse="sidebarOpen = false" @expand="sidebarOpen = true">
      <Sidebar collapsible="none" class="h-full min-h-0 w-full min-w-0 bg-transparent">
        <SidebarContent class="overflow-hidden">
          <SidebarGroup class="min-h-0 flex-1 px-1 py-2">
            <SidebarGroupContent class="flex min-h-0 flex-1 flex-col gap-2">
              <p class="px-2 text-xs font-medium text-muted-foreground">Collections</p>
              <InputGroup class="mx-1 h-8 w-auto">
                <InputGroupAddon align="inline-start">
                  <Search class="size-4 opacity-50" aria-hidden="true" />
                </InputGroupAddon>
                <InputGroupInput v-model="collectionQuery" data-testid="content-collection-filter" placeholder="Filter collections" aria-label="Filter collections" class="h-8" />
              </InputGroup>
              <nav data-testid="content-collections" aria-label="Collections" class="flex min-h-0 flex-1 flex-col gap-0.5 overflow-auto">
                <div v-for="c in filteredCollections" :key="c.name" class="group flex items-center gap-0.5">
                  <button
                    type="button"
                    class="min-w-0 flex-1 truncate rounded-md px-2 py-1.5 text-left text-sm outline-none hover:bg-accent/60 focus-visible:bg-accent"
                    :class="store.selectedCollection === c.name ? 'bg-accent text-accent-foreground' : ''"
                    :aria-current="store.selectedCollection === c.name ? 'page' : undefined"
                    data-testid="content-collection"
                    :data-collection-name="c.name"
                    @click="openCollection(c)"
                  >{{ humanize(c.name) }}</button>
                </div>
                <p v-if="!filteredCollections.length" class="px-2 py-1.5 text-xs text-muted-foreground" data-testid="content-collections-empty">No matches</p>
              </nav>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
      </Sidebar>
      </ResizablePanel>
      <ResizableHandle v-if="!editing" with-handle class="bg-transparent" />
      <ResizablePanel id="content-editor" :default-size="80" :min-size="40">
      <SidebarInset class="@container/inset h-full min-h-0 min-w-0 w-auto overflow-hidden bg-background">
      <div v-if="!editing" class="flex h-full min-h-0 flex-col">
        <div class="flex h-11 shrink-0 items-center px-2">
          <Button type="button" variant="ghost" size="icon" class="size-8 text-muted-foreground" :aria-label="sidebarOpen ? 'Hide collections' : 'Show collections'" :title="sidebarOpen ? 'Hide collections' : 'Show collections'" data-testid="content-toggle-sidebar" @click="toggleSidebar">
            <PanelLeft class="size-4" />
          </Button>
        </div>
        <p v-if="store.error" role="alert" class="px-4 pb-2 text-sm text-destructive">{{ store.error }}</p>
        <section v-if="store.needsChange" role="alert" class="mx-4 mb-2 rounded-md border p-3 text-sm" data-testid="content-needs-change">
          <p>Content cannot be edited on main. Create a change to {{ store.creation ? "create this entry" : "save your edits" }}.</p>
          <Button type="button" size="sm" class="mt-2" data-testid="content-create-change" @click="showNewChange = true">Create change</Button>
        </section>
        <EntryList
          v-if="activeCollection"
          class="min-h-0 flex-1"
          :collection="activeCollection"
          :entries="store.entries"
          :loading="store.loading"
          :create-hint="createBlocked(activeCollection)"
          :actions="rowActions"
          @open="openEntry(activeCollection.name, $event)"
          @create="newEntry(activeCollection.name)"
        />
        <div v-else-if="!store.loading && !store.error && !store.collections.length" class="flex h-full flex-col items-center justify-center gap-3 px-4 pb-11 text-center text-sm text-muted-foreground" data-testid="content-empty-catalog">
          <FileText class="size-8 opacity-50" aria-hidden="true" />
          <p class="font-medium text-foreground">No content collections</p>
          <p class="max-w-md">Content editing needs the orso-astro integration: <code>orso()</code> in astro.config and <code>orsoGlob</code> loaders in src/content.config.ts. Collections appear once the preview starts.</p>
        </div>
        <p v-else-if="store.loading" class="px-4 text-sm text-muted-foreground">Loading content…</p>
        <div v-else class="flex h-full flex-col items-center justify-center gap-3 px-4 pb-11 text-center text-sm text-muted-foreground" data-testid="content-choose-collection">
          <FileText class="size-8 opacity-50" aria-hidden="true" />
          <p>Choose a collection.</p>
        </div>
      </div>
      <form v-else ref="editorForm" class="relative flex h-full min-h-0 flex-col" :data-testid="store.entry && store.descriptor ? 'content-form' : undefined" @submit.prevent="store.entry && store.descriptor && !store.readOnlyReason && save()" @keydown.esc="history.selectedSha && history.select(null)">
      <header class="relative z-10 flex h-11 shrink-0 items-center gap-1 bg-background px-2">
        <div class="pointer-events-none absolute inset-x-0 top-full h-6 bg-linear-to-b from-background to-transparent" aria-hidden="true" />
        <Button type="button" variant="ghost" size="icon" class="size-8 text-muted-foreground" :aria-label="backLabel" :title="backLabel" data-testid="content-back" @click="backToList">
          <ChevronLeft class="size-4" />
        </Button>
        <template v-if="store.entry">
          <div class="flex min-w-0 items-center gap-1 px-1">
            <h2 class="truncate text-sm font-medium" data-testid="content-entry-title">{{ humanize(store.entry.id) }}</h2>
            <Button v-if="canRemove" type="button" variant="ghost" size="icon" class="size-7 shrink-0 text-muted-foreground" aria-label="Rename" :title="store.dirty ? 'Save or discard your changes first' : 'Rename'" data-testid="content-rename" :disabled="store.dirty" @click="store.startLifecycle('rename')"><Pencil class="size-3.5" /></Button>
            <p class="hidden truncate text-xs text-muted-foreground @xl/inset:block" :title="`${store.entry.path} · ${store.entry.branch || 'No branch'}`">{{ store.entry.branch || 'No branch' }}</p>
          </div>
          <div class="ml-auto flex shrink-0 items-center gap-1">
            <span v-if="wordCount !== null" class="hidden px-2 text-xs text-muted-foreground @lg/inset:inline" data-testid="content-word-count">{{ wordCount }} {{ wordCount === 1 ? "word" : "words" }}</span>
            <template v-if="canRemove">
              <Button type="button" variant="ghost" size="icon" class="size-8 text-muted-foreground hover:bg-destructive/10 hover:text-destructive" aria-label="Delete" data-testid="content-delete" :disabled="store.dirty" :title="store.dirty ? 'Save or discard your changes first' : 'Delete'" @click="store.startLifecycle('delete')"><Trash2 class="size-4" /></Button>
            </template>
            <Button v-if="hasFieldsPanel" type="button" variant="ghost" size="icon" class="size-8 text-muted-foreground" :aria-label="fieldsOpen ? 'Hide fields' : 'Show fields'" :title="fieldsOpen ? 'Hide fields' : 'Show fields'" :aria-pressed="fieldsOpen" data-testid="content-toggle-fields" @click="fieldsOpen = !fieldsOpen">
              <PanelRight class="size-4" />
            </Button>
          </div>
        </template>
      </header>
      <div class="min-h-0 flex-1 overflow-auto [container-type:size]">
      <div class="px-4">
        <p v-if="store.error" role="alert" class="text-sm text-destructive">{{ store.error }}</p>
        <p v-if="store.loading" class="text-sm text-muted-foreground">Loading content…</p>
        <section v-if="store.needsChange" role="alert" class="mb-3 rounded-md border p-3 text-sm" data-testid="content-needs-change">
          <p>Content cannot be edited on main. Create a change to {{ store.creation ? "create this entry" : "save your edits" }}.</p>
          <Button type="button" size="sm" class="mt-2" data-testid="content-create-change" @click="showNewChange = true">Create change</Button>
        </section>
        <p v-if="store.entry && store.changedOnDisk" class="mb-3 text-sm" role="status">Content changed on disk. Your unsaved draft is preserved.</p>
        <template v-if="store.entry && store.descriptor">
          <p v-if="store.readOnlyReason" class="mb-3 text-sm text-muted-foreground" role="status" data-testid="content-read-only">{{ store.readOnlyReason }}</p>
          <p v-if="store.saveError" role="alert" class="mb-2 text-sm text-destructive" data-testid="content-save-error">{{ store.saveError }}</p>
          <ul v-if="Object.keys(store.fieldErrors).length" class="mb-2 text-xs text-destructive" data-testid="content-field-errors">
            <li v-for="(msg, path) in store.fieldErrors" :key="path">{{ path }}: {{ msg }}</li>
          </ul>
          <section v-if="store.conflict" role="alert" class="mb-3 rounded-md border border-destructive p-3 text-sm" data-testid="content-conflict">
            <p>{{ store.conflict.message }}. Your edits are kept.<template v-if="store.conflict.branch"> Reload and Compare use {{ store.conflict.branch }}.</template></p>
            <div class="mt-2 flex gap-2">
              <Button type="button" variant="outline" size="sm" data-testid="content-reload" @click="store.reload()">Reload entry</Button>
              <Button type="button" variant="outline" size="sm" data-testid="content-compare" @click="store.compare()">Compare</Button>
            </div>
            <table v-if="store.current" class="mt-2 w-full text-xs" data-testid="content-compare-table">
              <thead><tr><th class="text-left">Field</th><th class="text-left">Your draft</th><th class="text-left">On disk now</th></tr></thead>
              <tbody>
                <tr v-for="row in differences" :key="row.name"><td>{{ row.name }}</td><td>{{ show(row.draft) }}</td><td>{{ show(row.current) }}</td></tr>
                <tr v-if="!differences.length"><td colspan="3">No differences from the current file.</td></tr>
              </tbody>
            </table>
          </section>
        </template>
      </div>
      <template v-if="store.entry">
        <div v-if="store.descriptor" class="@container flex min-h-full">
          <div class="flex min-h-full w-full flex-col @3xl:flex-row">
          <aside v-show="!hasFieldsPanel || fieldsOpen" class="min-w-0 px-4 pt-6 pb-4 @3xl:sticky @3xl:top-0 @3xl:order-2 @3xl:max-h-[100cqh] @3xl:w-80 @3xl:shrink-0 @3xl:self-start @3xl:overflow-y-auto" :class="{ 'order-2': !hasFieldsPanel }" data-testid="content-side">
            <Tabs :model-value="sideTab" @update:model-value="tab = $event as 'fields' | 'history'">
              <TabsList class="w-full">
                <TabsTrigger v-if="hasFieldsPanel" value="fields">Fields</TabsTrigger>
                <TabsTrigger value="history">History</TabsTrigger>
              </TabsList>
              <TabsContent v-if="hasFieldsPanel" value="fields" data-testid="content-fields">
                <div v-if="canRenameEntry" class="mb-4 space-y-1" data-testid="content-slug-field">
                  <Label for="content-slug" class="text-xs font-medium">Slug<span class="text-destructive">*</span></Label>
                  <div class="flex gap-1">
                    <Input
                      v-if="store.creating"
                      id="content-slug"
                      :model-value="store.creation?.slug"
                      data-testid="content-slug"
                      required
                      aria-required="true"
                      class="h-8"
                      @update:model-value="store.setCreateSlug(String($event))"
                    />
                    <Input v-else id="content-slug" v-model="slugDraft" data-testid="content-slug" required aria-required="true" class="h-8" @keydown.enter.prevent="commitSlug" />
                    <Button v-if="!store.creating && slugDirty" type="button" size="sm" data-testid="content-slug-apply" :disabled="store.saving || !slugDraft.trim()" @click="commitSlug">Rename</Button>
                  </div>
                  <p class="text-xs text-muted-foreground">{{ store.creating ? "The page's address. Nothing is saved until you save." : "The page's address. Renaming moves the file and updates links." }}</p>
                </div>
                <FrontmatterForm :descriptor="store.descriptor" :model-value="store.draftFields" :errors="store.fieldErrors" @update:model-value="store.draftFields = $event as Record<string, unknown>" />
              </TabsContent>
              <TabsContent value="history">
                <VersionsList :versions="history.versions" :selected-sha="history.selectedSha" :loading="history.loading" :error="history.error" @select="history.select($event)" @retry="history.reload()" />
              </TabsContent>
            </Tabs>
          </aside>
          <div class="min-w-0 flex-1 px-4 pt-4 pb-8 @3xl:order-1 @3xl:px-6">
          <div class="mx-auto w-full" :class="{ 'max-w-3xl': !history.version }">
          <VersionDiff v-if="history.version" :path="history.version.path" :old-source="history.version.source" :new-source="store.entry.source" :dirty="versionDirty" :can-restore="!restoreReason" :restore-reason="restoreReason" @back="history.select(null)" @restore="restore" />
          <MarkdownBodyEditor v-else-if="hasFieldsPanel" :key="`${store.entry.collection}/${store.entry.id}`" :body="store.draftBody" :original="store.entry.body ?? ''" :kind="store.entry.kind as 'markdown' | 'mdx'" :rich="store.richBody" :resolve-image="resolveImage" @change="store.setBody($event.body)" />
          <FrontmatterForm v-else data-testid="content-fields" :descriptor="store.descriptor" :model-value="store.draftFields" :errors="store.fieldErrors" @update:model-value="store.draftFields = $event as Record<string, unknown>" />
          </div>
          </div>
          </div>
        </div>
        <pre v-else data-testid="content-source" class="mx-auto max-w-3xl whitespace-pre-wrap break-words px-6 py-4 text-sm">{{ store.draftSource }}</pre>
      </template>
      <div v-if="store.entry && store.descriptor" class="h-24" aria-hidden="true" />
      </div>
      <Transition enter-active-class="transition duration-150 ease-out" enter-from-class="translate-y-2 opacity-0" leave-active-class="transition duration-100 ease-in" leave-to-class="translate-y-2 opacity-0">
        <div v-if="store.creating || (store.entry && store.descriptor && (store.dirty || store.saving))" role="status" class="absolute bottom-4 z-20 w-[min(28rem,calc(100%-2rem))] -translate-x-1/2" :style="{ left: `${unsavedCentre}px` }" data-testid="content-unsaved">
          <div class="flex w-full items-center gap-2 rounded-xl border bg-popover py-2 pr-2 pl-4 text-popover-foreground shadow-lg">
            <span class="size-2 shrink-0 rounded-full bg-amber-500" aria-hidden="true" />
            <span class="min-w-0 flex-1 truncate text-sm">Unsaved changes</span>
            <Button type="button" variant="ghost" size="sm" data-testid="content-discard" :disabled="store.saving" @click="store.discard()">Discard</Button>
            <Button type="submit" size="sm" data-testid="content-save" :disabled="store.creating ? !!store.readOnlyReason : !store.dirty || store.saving || !!store.readOnlyReason" :title="store.readOnlyReason ?? undefined">{{ store.saving ? "Saving…" : "Save" }}</Button>
          </div>
        </div>
      </Transition>
      </form>
        <AlertDialog v-model:open="lifecycleOpen">
          <AlertDialogContent v-if="store.lifecycle" data-testid="content-lifecycle">
            <AlertDialogHeader>
              <AlertDialogTitle>{{ store.lifecycle.action === "rename" ? "Rename" : "Delete" }} {{ store.lifecycle.collection }}/{{ store.lifecycle.id }}?</AlertDialogTitle>
              <AlertDialogDescription>
                <template v-if="store.lifecycle.action === 'rename'">The entry file, its public URL, and its asset folder move to the new slug. References below are updated too.</template>
                <template v-else>The entry and its asset folder are removed. History can restore them.</template>
              </AlertDialogDescription>
            </AlertDialogHeader>
            <p v-if="!store.lifecycle.impact && !store.lifecycle.error" class="text-sm text-muted-foreground">Checking references…</p>
            <div v-if="store.lifecycle.impact" class="space-y-3 text-sm">
              <section>
                <h3 class="font-medium">References</h3>
                <p v-if="store.lifecycle.action === 'delete' && store.lifecycle.impact.references.length" class="text-destructive">Remove these references before deleting.</p>
                <ul><li v-for="r in store.lifecycle.impact.references" :key="r.path + r.field">{{ r.collection }}/{{ r.id }} · {{ r.field }}</li></ul>
                <p v-if="!store.lifecycle.impact.references.length" class="text-muted-foreground">None</p>
              </section>
              <section>
                <h3 class="font-medium">Assets</h3>
                <ul><li v-for="a in store.lifecycle.impact.assets" :key="a" class="break-all">{{ a }}</li></ul>
                <p v-if="!store.lifecycle.impact.assets.length" class="text-muted-foreground">None</p>
              </section>
              <section v-if="store.lifecycle.impact.proseLinks.length">
                <h3 class="font-medium">Links in text (review by hand)</h3>
                <ul><li v-for="link in store.lifecycle.impact.proseLinks" :key="link.path + link.line">{{ link.path }}:{{ link.line }} → {{ link.url }}</li></ul>
              </section>
              <label v-if="store.lifecycle.action === 'rename'" class="block text-xs font-medium">New slug
                <Input v-model="store.lifecycle.slug" data-testid="content-rename-slug" class="mt-1" placeholder="new-slug" />
              </label>
            </div>
            <p v-if="store.lifecycle.error" role="alert" class="text-sm text-destructive" data-testid="content-lifecycle-error">{{ store.lifecycle.error }}</p>
            <AlertDialogFooter>
              <AlertDialogCancel data-testid="content-lifecycle-cancel">Cancel</AlertDialogCancel>
              <Button :variant="store.lifecycle.action === 'delete' ? 'destructive' : 'default'" :disabled="!canConfirm" data-testid="content-lifecycle-confirm" @click="confirmLifecycle">
                {{ store.lifecycle.busy ? "Working…" : store.lifecycle.action === "rename" ? "Rename" : "Delete" }}
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      <AlertDialog v-model:open="confirmRestore">
        <AlertDialogContent data-testid="content-restore-confirm">
          <AlertDialogHeader>
            <AlertDialogTitle>Replace unsaved edits?</AlertDialogTitle>
            <AlertDialogDescription>Your unsaved edits to this entry are replaced by this version. Nothing is saved until you choose Save.</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <Button variant="destructive" @click="applyRestore">Replace</Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <NewChangeDialog v-model:open="showNewChange" :project-id="props.projectId" @created="resume" />
      </SidebarInset>
      </ResizablePanel>
      </ResizablePanelGroup>
    </SidebarProvider>
  </div>
</template>
