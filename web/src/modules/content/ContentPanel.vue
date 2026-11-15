<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, provide, ref, watch } from "vue";
import { useElementBounding, useWindowSize } from "@vueuse/core";
import { toast } from "@/lib/toast";
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRouter } from "vue-router";
import type { ModalProps, SplitterItem, TableColumn, TabsItem } from "@nuxt/ui";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UCard from "@nuxt/ui/components/Card.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UFormField from "@nuxt/ui/components/FormField.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import USplitter from "@nuxt/ui/components/Splitter.vue";
import UTable from "@nuxt/ui/components/Table.vue";
import UTabs from "@nuxt/ui/components/Tabs.vue";
import { contentPath } from "@/router";
import { useProjectsStore } from "@/modules/projects/store";
import { openInPreview } from "./openInPreview";
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
const projects = useProjectsStore();
const daemon = useDaemon();
const changes = useChangesStore();
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
const openEntryCollection = computed(() => store.collections.find(c => c.name === store.entry?.collection));
const previewReady = computed(
  () => projects.projects.find((p) => p.id === props.projectId)?.devserver === "ready",
);
const canOpenInPreview = computed(() => {
  const c = openEntryCollection.value;
  return !!store.entry && c?.hasPreviewPath === true && previewReady.value;
});
function previewEntry() {
  const e = store.entry;
  if (!e || !canOpenInPreview.value) return;
  void openInPreview({
    projectId: props.projectId,
    collection: e.collection,
    id: e.id,
    resolve: daemon.api.resolveContentPreviewPath,
    previewBaseUrl: daemon.api.previewUrl(props.projectId),
  });
}
const lifecycleOpen = computed({
  // Hidden while waiting for Create Change so the form's prompt is reachable.
  get: () => !!store.lifecycle && !store.needsChange,
  set: open => { if (!open) store.cancelLifecycle(); },
});
const canConfirm = computed(() => {
  const l = store.lifecycle;
  if (!l?.impact || l.busy) return false;
  return !l.impact.references.length;
});
async function confirmLifecycle() { try { await store.confirmLifecycle(); showInUrl(); } catch { /* recorded on the store; the draft stays */ } }
/** The branch the project is on, as the Changes picker knows it. */
const currentBranch = () => changes.active?.branch ?? "main";
/** The collections sidebar and the editor; panel ids and the auto-save id match the old resizable layout. */
const splitter = ref<{ panelsRef: ({ collapse?: () => void; expand?: () => void } | undefined)[] } | null>(null);
const COLLECTIONS_PANEL: SplitterItem = { id: "content-collections", slot: "collections", order: 1, defaultSize: 16, minSize: 10, maxSize: 30, collapsible: true, collapsedSize: 0 };
const EDITOR_PANEL: SplitterItem = { id: "content-editor", slot: "editor", order: 2, defaultSize: 80, minSize: 40 };
/** The sidebar's filter; it matches the name the writer reads, not the raw collection id. */
const collectionQuery = ref("");
const filteredCollections = computed(() => {
  const q = collectionQuery.value.trim().toLowerCase();
  return q ? validCollections.value.filter(c => humanize(c.name).toLowerCase().includes(q)) : validCollections.value;
});
const sidebarOpen = ref(true);
const fieldsOpen = ref(true);
function toggleSidebar() {
  const panel = splitter.value?.panelsRef[0];
  if (sidebarOpen.value) panel?.collapse?.(); else panel?.expand?.();
}
const panels = computed<SplitterItem[]>(() => (editing.value ? [EDITOR_PANEL] : [COLLECTIONS_PANEL, EDITOR_PANEL]));
function panelToggled(index: number, open: boolean) { if (!editing.value && index === 0) sidebarOpen.value = open; }
/** Markdown and MDX entries edit the body in the page and the fields beside it. */
const hasFieldsPanel = computed(() => !!store.descriptor && !!store.entry && store.entry.kind !== "json");
const history = useContentHistoryStore();
const tab = ref<"fields" | "history">("fields");
/** JSON entries edit their fields in the page, so their side panel only has History. */
const sideTab = computed(() => (hasFieldsPanel.value ? tab.value : "history"));
const sideTabs = computed<TabsItem[]>(() => [
  ...(hasFieldsPanel.value ? [{ label: "Fields", icon: "i-hugeicons-text-creation", value: "fields", slot: "fields" }] : []),
  { label: "History", icon: "i-hugeicons-clock-04", value: "history", slot: "history" },
]);
const entryKey = computed(() => (store.entry ? `${store.entry.collection}/${store.entry.id}` : null));
/** The URL slug is the entry's file name; saving a new one renames the document. */
const slugDraft = ref("");
const slugDirty = computed(() => !!store.entry && slugDraft.value.trim() !== store.entry.id);
/** A collection that declares its own `slug` field uses that as the address, so do not show two. */
const schemaHasSlug = computed(() => !!store.descriptor?.properties?.slug);
const canRenameEntry = computed(() => !!store.entry && removable(store.entry.collection) && !schemaHasSlug.value);
watch(entryKey, () => { slugDraft.value = store.entry?.id ?? ""; }, { immediate: true });
/** Rename from the slug field; a dirty draft is saved first, and Create Change resumes it. */
async function commitSlug() {
  const entry = store.entry;
  const next = slugDraft.value.trim();
  if (!entry || !next || next === entry.id) { slugDraft.value = entry?.id ?? ""; return; }
  try { await store.renameEntry(next); showInUrl(); }
  catch { /* recorded on the store; the draft stays */ }
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
/** Open the first listed collection when the catalog arrives with nothing selected. */
function selectDefaultCollection(collections: ContentCollection[]) {
  if (props.collection || props.entry || store.selectedCollection) return;
  const first = collections[0];
  if (first) void router.replace(contentPath(props.projectId, first.name, first.singleton ?? undefined));
}
/** A deep link to a singleton collection has no list to show: send it to its one entry. */
watch(validCollections, collections => {
  selectDefaultCollection(collections);
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
/** The row menu: the entry opens, shares, or deletes exactly as it did in the tree. */
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
      remove: () => void deleteFor(collection, id),
    },
  });
}
/** The URL names the open collection and entry; without one, restore the last list or open the first. */
async function follow(collection?: string, id?: string) {
  if (!collection) {
    const remembered = store.selectedCollection;
    if (store.projectId === props.projectId && remembered && !singletonOf(remembered)) {
      void router.replace(contentPath(props.projectId, remembered));
      return;
    }
    selectDefaultCollection(validCollections.value);
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
/** Opens the entry, then starts its delete; leaving a draft is confirmed first. */
async function deleteFor(collection: string, id: string) {
  await router.push(contentPath(props.projectId, collection, id));
  await follow(collection, id);
  if (store.entry?.collection === collection && store.entry.id === id) void store.startLifecycle();
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
/** Nuxt UI passes extra `content` keys to the dialog panel as attributes; this one names it for tests. */
const dialogTestId = (id: string) => ({ "data-testid": id }) as ModalProps["content"];
type Difference = { name: string; draft: unknown; current: unknown };
const differenceColumns: TableColumn<Difference>[] = [
  { accessorKey: "name", header: "Field" },
  { id: "draft", header: "Your draft", cell: ({ row }) => show(row.original.draft) },
  { id: "current", header: "On disk now", cell: ({ row }) => show(row.original.current) },
];
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
    <USplitter
      ref="splitter"
      :items="panels"
      auto-save-id="content-sidebar"
      class="h-full min-h-0 overflow-hidden"
      :ui="{ handle: 'flex items-center justify-center' }"
      @collapse="panelToggled($event, false)"
      @expand="panelToggled($event, true)"
    >
      <template #collections>
        <nav class="bg-background flex h-full min-h-0 w-full min-w-0 flex-col gap-2 px-1.5 py-2" aria-label="Content collections" data-testid="content-collections-sidebar">
          <UFormField label="Collections" :ui="{ label: 'text-dimmed text-xs font-normal tracking-wide px-1', container: 'mt-2' }" class="shrink-0">
            <UInput
              v-model="collectionQuery"
              icon="i-hugeicons-search-01"
              size="sm"
              class="w-full"
              data-testid="content-collection-filter"
              placeholder="Filter collections"
              aria-label="Filter collections"
            />
          </UFormField>
          <ul data-testid="content-collections" aria-label="Collections" class="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto p-0.5">
            <li v-for="c in filteredCollections" :key="c.name">
              <UButton
                type="button"
                color="neutral"
                :variant="store.selectedCollection === c.name ? 'soft' : 'ghost'"
                size="sm"
                block
                icon="i-hugeicons-folder-01"
                :label="humanize(c.name)"
                class="justify-start"
                :class="store.selectedCollection === c.name ? 'font-medium' : 'text-default'"
                :aria-current="store.selectedCollection === c.name ? 'page' : undefined"
                data-testid="content-collection"
                :data-collection-name="c.name"
                @click="openCollection(c)"
              />
            </li>
            <li v-if="!filteredCollections.length" class="text-dimmed px-3 py-2 text-xs" data-testid="content-collections-empty">No matches</li>
          </ul>
        </nav>
      </template>
      <template #resize-handle>
        <div
          class="bg-accented group-hover:bg-inverted/70 group-data-[resize-handle-state=drag]:bg-inverted h-8 w-1 rounded-full transition-colors"
          aria-hidden="true"
        />
      </template>
      <template #editor>
      <div class="@container/inset bg-background h-full min-h-0 w-full min-w-0 overflow-hidden">
      <div v-if="!editing" class="flex h-full min-h-0 flex-col">
        <div class="flex h-11 shrink-0 items-center px-2">
          <UButton type="button" color="neutral" variant="ghost" square icon="i-hugeicons-sidebar-left" class="text-muted" :aria-label="sidebarOpen ? 'Hide collections' : 'Show collections'" :title="sidebarOpen ? 'Hide collections' : 'Show collections'" data-testid="content-toggle-sidebar" @click="toggleSidebar" />
        </div>
        <UAlert v-if="store.error" role="alert" color="error" variant="subtle" icon="i-hugeicons-alert-circle" :description="store.error" class="mx-4 mb-2 w-auto" />
        <UAlert v-if="store.needsChange" role="alert" color="neutral" variant="outline" orientation="horizontal" class="mx-4 mb-2 w-auto" data-testid="content-needs-change" :description="`Content cannot be edited on the live site. Create a change to ${store.creation ? 'create this entry' : 'save your edits'}.`">
          <template #actions>
            <UButton type="button" size="sm" label="Create change" data-testid="content-create-change" @click="showNewChange = true" />
          </template>
        </UAlert>
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
        <UEmpty v-else-if="!store.loading && !store.error && !store.collections.length" variant="naked" icon="i-hugeicons-file-01" title="No content collections" class="h-full pb-11" data-testid="content-empty-catalog">
          <template #description>
            Content editing needs the orso-astro integration: <code>orso()</code> in astro.config and <code>orsoGlob</code> loaders in src/content.config.ts. Collections appear once the preview starts.
          </template>
        </UEmpty>
        <p v-else-if="store.loading" class="text-muted px-4 text-sm">Loading content…</p>
        <UEmpty v-else variant="naked" icon="i-hugeicons-file-01" description="Choose a collection." class="h-full pb-11" data-testid="content-choose-collection" />
      </div>
      <form v-else ref="editorForm" class="relative flex h-full min-h-0 flex-col" :data-testid="store.entry && store.descriptor ? 'content-form' : undefined" @submit.prevent="store.entry && store.descriptor && !store.readOnlyReason && save()" @keydown.esc="history.selectedSha && history.select(null)">
      <header class="bg-background relative z-10 flex h-11 shrink-0 items-center gap-1 px-2">
        <div class="from-background pointer-events-none absolute inset-x-0 top-full h-6 bg-linear-to-b to-transparent" aria-hidden="true" />
        <UButton type="button" color="neutral" variant="outline" square icon="i-hugeicons-arrow-left-01" :aria-label="backLabel" :title="backLabel" data-testid="content-back" @click="backToList" />
        <template v-if="store.entry">
          <div class="flex min-w-0 items-center gap-1 px-1">
            <h2 class="text-highlighted truncate text-sm font-medium" data-testid="content-entry-title">{{ humanize(store.entry.id) }}</h2>
            <p class="text-muted hidden truncate text-xs @xl/inset:block" :title="`${store.entry.path} · ${store.entry.branch || 'No branch'}`">{{ store.entry.branch || 'No branch' }}</p>
          </div>
          <div class="ml-auto flex shrink-0 items-center gap-1">
            <span v-if="wordCount !== null" class="text-muted hidden px-2 text-xs @lg/inset:inline" data-testid="content-word-count">{{ wordCount }} {{ wordCount === 1 ? "word" : "words" }}</span>
            <UButton
              v-if="openEntryCollection?.hasPreviewPath === true"
              type="button"
              color="neutral"
              variant="outline"
              icon="i-hugeicons-link-square-02"
              label="Open preview"
              data-testid="content-open-preview"
              :title="previewReady ? 'Open in new tab' : 'Start the preview to open this entry'"
              :disabled="!previewReady || !store.entry"
              @click="previewEntry"
            />
            <UButton v-if="canRemove" type="button" color="neutral" variant="ghost" square icon="i-hugeicons-delete-02" class="text-muted hover:text-error hover:bg-error/10" aria-label="Delete" data-testid="content-delete" :disabled="store.dirty" :title="store.dirty ? 'Save or discard your changes first' : 'Delete'" @click="store.startLifecycle()" />
            <UButton v-if="hasFieldsPanel" type="button" color="neutral" variant="ghost" square icon="i-hugeicons-sidebar-right" class="text-muted" :aria-label="fieldsOpen ? 'Hide fields' : 'Show fields'" :title="fieldsOpen ? 'Hide fields' : 'Show fields'" :aria-pressed="fieldsOpen" data-testid="content-toggle-fields" @click="fieldsOpen = !fieldsOpen" />
          </div>
        </template>
      </header>
      <div class="relative min-h-0 flex-1">
      <div class="h-full overflow-auto [container-type:size]">
      <div class="flex flex-col gap-3 px-4 empty:hidden">
        <UAlert v-if="store.error" role="alert" color="error" variant="subtle" icon="i-hugeicons-alert-circle" :description="store.error" />
        <p v-if="store.loading" class="text-muted text-sm">Loading content…</p>
        <UAlert v-if="store.needsChange" role="alert" color="neutral" variant="outline" orientation="horizontal" data-testid="content-needs-change" :description="`Content cannot be edited on the live site. Create a change to ${store.creation ? 'create this entry' : 'save your edits'}.`">
          <template #actions>
            <UButton type="button" size="sm" label="Create change" data-testid="content-create-change" @click="showNewChange = true" />
          </template>
        </UAlert>
        <UAlert v-if="store.entry && store.changedOnDisk" role="status" color="warning" variant="subtle" icon="i-hugeicons-information-circle" description="Content changed on disk. Your unsaved draft is preserved." />
        <template v-if="store.entry && store.descriptor">
          <UAlert v-if="store.readOnlyReason" role="status" color="neutral" variant="subtle" icon="i-hugeicons-square-lock-02" :description="store.readOnlyReason" data-testid="content-read-only" />
          <UAlert v-if="store.saveError" role="alert" color="error" variant="subtle" icon="i-hugeicons-alert-circle" :description="store.saveError" data-testid="content-save-error" />
          <UAlert v-if="Object.keys(store.fieldErrors).length" color="error" variant="subtle" data-testid="content-field-errors">
            <template #description>
              <ul class="text-xs">
                <li v-for="(msg, path) in store.fieldErrors" :key="path">{{ path }}: {{ msg }}</li>
              </ul>
            </template>
          </UAlert>
          <UAlert v-if="store.conflict" role="alert" color="error" variant="outline" icon="i-hugeicons-git-compare" data-testid="content-conflict">
            <template #description>
              <p>{{ store.conflict.message }}. Your edits are kept.<template v-if="store.conflict.branch"> Reload and Compare use {{ store.conflict.branch }}.</template></p>
              <div class="mt-2 flex gap-2">
                <UButton type="button" color="neutral" variant="outline" size="sm" label="Reload entry" data-testid="content-reload" @click="store.reload()" />
                <UButton type="button" color="neutral" variant="outline" size="sm" label="Compare" data-testid="content-compare" @click="store.compare()" />
              </div>
              <UTable v-if="store.current" :data="differences" :columns="differenceColumns" empty="No differences from the current file." class="mt-2 text-xs" data-testid="content-compare-table" />
            </template>
          </UAlert>
        </template>
      </div>
      <template v-if="store.entry">
        <div v-if="store.descriptor" class="@container flex min-h-full">
          <div class="flex min-h-full w-full flex-col @3xl:flex-row">
          <aside v-show="!hasFieldsPanel || fieldsOpen" class="min-w-0 px-4 pt-6 pb-4 @3xl:sticky @3xl:top-0 @3xl:order-2 @3xl:max-h-[100cqh] @3xl:w-80 @3xl:shrink-0 @3xl:self-start @3xl:overflow-y-auto" :class="{ 'order-2': !hasFieldsPanel }" data-testid="content-side">
            <UTabs :items="sideTabs" :model-value="sideTab" size="sm" color="neutral" variant="pill" class="w-full" :ui="{ root: 'items-stretch', trigger: 'flex-1 justify-center data-[state=active]:text-highlighted' }" @update:model-value="tab = $event as 'fields' | 'history'">
              <template #fields>
                <div data-testid="content-fields">
                  <UFormField
                    v-if="canRenameEntry"
                    label="Slug"
                    required
                    :help="store.creating ? 'The page\'s address. Nothing is saved until you save.' : 'The page\'s address. Renaming moves the file and updates links.'"
                    :ui="{ label: 'text-xs' }"
                    class="mb-4"
                    data-testid="content-slug-field"
                  >
                    <div class="flex gap-1">
                      <UInput
                        v-if="store.creating"
                        id="content-slug"
                        :model-value="store.creation?.slug"
                        data-testid="content-slug"
                        required
                        aria-required="true"
                        size="sm"
                        class="flex-1"
                        @update:model-value="store.setCreateSlug(String($event))"
                      />
                      <UInput v-else id="content-slug" v-model="slugDraft" data-testid="content-slug" required aria-required="true" size="sm" class="flex-1" @keydown.enter.prevent="commitSlug" />
                      <UButton v-if="!store.creating && slugDirty" type="button" size="sm" label="Rename" data-testid="content-slug-apply" :disabled="store.saving || !slugDraft.trim()" @click="commitSlug" />
                    </div>
                  </UFormField>
                  <FrontmatterForm :descriptor="store.descriptor" :model-value="store.draftFields" :errors="store.fieldErrors" @update:model-value="store.draftFields = $event as Record<string, unknown>" />
                </div>
              </template>
              <template #history>
                <VersionsList :versions="history.versions" :selected-sha="history.selectedSha" :loading="history.loading" :error="history.error" @select="history.select($event)" @retry="history.reload()" />
              </template>
            </UTabs>
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
        <pre v-else data-testid="content-source" class="text-default mx-auto max-w-3xl px-6 py-4 text-sm break-words whitespace-pre-wrap">{{ store.draftSource }}</pre>
      </template>
      <div v-if="store.entry && store.descriptor" class="h-24" aria-hidden="true" />
      </div>
      <div class="from-background via-background/60 pointer-events-none absolute inset-x-0 bottom-0 z-10 h-10 bg-linear-to-t to-transparent" aria-hidden="true" />
      </div>
      <Transition enter-active-class="transition duration-150 ease-out" enter-from-class="translate-y-2 opacity-0" leave-active-class="transition duration-100 ease-in" leave-to-class="translate-y-2 opacity-0">
        <div v-if="store.creating || (store.entry && store.descriptor && (store.dirty || store.saving))" role="status" class="absolute bottom-4 z-20 w-[min(28rem,calc(100%-2rem))] -translate-x-1/2" :style="{ left: `${unsavedCentre}px` }" data-testid="content-unsaved">
          <UCard class="rounded-xl shadow-lg" :ui="{ body: 'flex w-full items-center gap-2 py-2 pr-2 pl-4 sm:py-2 sm:pr-2 sm:pl-4' }">
            <span class="bg-warning size-2 shrink-0 rounded-full" aria-hidden="true" />
            <span class="text-highlighted min-w-0 flex-1 truncate text-sm">Unsaved changes</span>
            <UButton type="button" color="neutral" variant="ghost" size="sm" label="Discard" data-testid="content-discard" :disabled="store.saving" @click="store.discard()" />
            <UButton type="submit" size="sm" :label="store.saving ? 'Saving…' : 'Save'" data-testid="content-save" :disabled="store.creating ? !!store.readOnlyReason : !store.dirty || store.saving || !!store.readOnlyReason" :title="store.readOnlyReason ?? undefined" />
          </UCard>
        </div>
      </Transition>
      </form>
      <UModal
        v-model:open="lifecycleOpen"
        :title="store.lifecycle ? `Delete ${store.lifecycle.collection}/${store.lifecycle.id}?` : undefined"
        description="The entry and its asset folder are removed. History can restore them."
        :close="false"
        :content="dialogTestId('content-lifecycle')"
      >
        <template #body>
          <div v-if="store.lifecycle" class="space-y-3 text-sm">
            <p v-if="!store.lifecycle.impact && !store.lifecycle.error" class="text-muted">Checking references…</p>
            <template v-if="store.lifecycle.impact">
              <section>
                <h3 class="text-highlighted font-medium">References</h3>
                <p v-if="store.lifecycle.impact.references.length" class="text-error">Remove these references before deleting.</p>
                <ul><li v-for="r in store.lifecycle.impact.references" :key="r.path + r.field">{{ r.collection }}/{{ r.id }} · {{ r.field }}</li></ul>
                <p v-if="!store.lifecycle.impact.references.length" class="text-muted">None</p>
              </section>
              <section>
                <h3 class="text-highlighted font-medium">Assets</h3>
                <ul><li v-for="a in store.lifecycle.impact.assets" :key="a" class="break-all">{{ a }}</li></ul>
                <p v-if="!store.lifecycle.impact.assets.length" class="text-muted">None</p>
              </section>
              <section v-if="store.lifecycle.impact.proseLinks.length">
                <h3 class="text-highlighted font-medium">Links in text (review by hand)</h3>
                <ul><li v-for="link in store.lifecycle.impact.proseLinks" :key="link.path + link.line">{{ link.path }}:{{ link.line }} → {{ link.url }}</li></ul>
              </section>
            </template>
            <UAlert v-if="store.lifecycle.error" role="alert" color="error" variant="subtle" :description="store.lifecycle.error" data-testid="content-lifecycle-error" />
          </div>
        </template>
        <template #footer>
          <div class="flex w-full justify-end gap-2">
            <UButton type="button" color="neutral" variant="outline" label="Cancel" data-testid="content-lifecycle-cancel" @click="lifecycleOpen = false" />
            <UButton type="button" color="error" :label="store.lifecycle?.busy ? 'Working…' : 'Delete'" :disabled="!canConfirm" data-testid="content-lifecycle-confirm" @click="confirmLifecycle" />
          </div>
        </template>
      </UModal>
      <UModal
        v-model:open="confirmRestore"
        title="Replace unsaved edits?"
        description="Your unsaved edits to this entry are replaced by this version. Nothing is saved until you choose Save."
        :close="false"
        :content="dialogTestId('content-restore-confirm')"
      >
        <template #footer>
          <div class="flex w-full justify-end gap-2">
            <UButton type="button" color="neutral" variant="outline" label="Cancel" @click="confirmRestore = false" />
            <UButton type="button" color="error" label="Replace" @click="applyRestore" />
          </div>
        </template>
      </UModal>
      <NewChangeDialog v-model:open="showNewChange" :project-id="props.projectId" @created="resume" />
      </div>
      </template>
    </USplitter>
  </div>
</template>
