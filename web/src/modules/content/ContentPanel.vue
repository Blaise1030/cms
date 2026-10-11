<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, provide, ref, watch } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate } from "vue-router";
import { Button } from "@/components/ui/button";
import { AlertDialog, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { useDaemon } from "@/core/daemon";
import NewChangeDialog from "@/modules/changes/NewChangeDialog.vue";
import { useChangesStore } from "@/modules/changes/store";
import type { Change } from "@/core/types";
import FrontmatterForm from "./FrontmatterForm.vue";
import { sameValue } from "./draft";
import { slugify, startingFields } from "./create";
import { entryAssetPath } from "./markdown";
import { useContentStore } from "./store";
import { IMAGE_REGISTRY } from "./ImageDialog.vue";
// Tiptap and CodeMirror load with the first Markdown/MDX entry, not with the app shell.
const MarkdownBodyEditor = defineAsyncComponent(() => import("./MarkdownBodyEditor.vue"));
const props = defineProps<{projectId: string}>();
const store = useContentStore();
const changes = useChangesStore();
const daemon = useDaemon();
// Images inserted by the form or body editor stay pending in the draft until Save.
provide(IMAGE_REGISTRY, { add: image => store.addImage(image) });
watch(() => props.projectId, id => void store.open(id), {immediate:true});
onBeforeRouteLeave(() => store.canLeave());
onBeforeRouteUpdate(() => store.canLeave());
const showNewChange = ref(false);
async function save() { try { await store.saveDraft(); } catch { /* recorded on the store; the draft stays */ } }
/** `site` is update-only and `pages/home` is required: neither can be renamed or deleted. */
const canRemove = computed(() => {
  const e = store.entry; if (!e || !store.descriptor) return false;
  const c = store.collections.find(x => x.name === e.collection);
  return !c?.singleton && !(e.collection === "pages" && e.id === "home");
});
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
async function confirmLifecycle() { try { await store.confirmLifecycle(); } catch { /* recorded on the store; the draft stays */ } }
/** The branch the project is on, as the Changes picker knows it. */
const currentBranch = () => changes.active?.branch ?? "main";
const selected = computed(() => store.collections.find(c => c.name === store.selectedCollection) ?? null);
/** Create is offered for multi-entry collections; this is why it is disabled, or null when it is not. */
const createBlocked = computed<string | null>(() => {
  const c = selected.value; if (!c || c.singleton) return null;
  if (!c.editable || !c.fields) return c.reason ?? "This collection is read only";
  const start = startingFields(c.fields, "");
  return "reason" in start ? start.reason : null;
});
const createOpen = computed({
  get: () => !!store.creation && !store.needsChange,
  set: open => { if (!open) store.cancelCreate(); },
});
/** The slug follows the title until the writer edits it. */
const slugEdited = ref(false);
function startCreate() { slugEdited.value = false; if (selected.value) store.startCreate(selected.value.name); }
function setTitle(title: string) { const c = store.creation; if (!c) return; c.title = title; if (!slugEdited.value) c.slug = slugify(title); }
function setSlug(slug: string) { const c = store.creation; if (!c) return; c.slug = slug; slugEdited.value = true; }
const canCreate = computed(() => { const c = store.creation; return !!c && !c.busy && !!c.title.trim() && !!c.slug.trim(); });
async function confirmCreate() { try { await store.confirmCreate(currentBranch()); } catch { /* recorded on the store */ } }
/** Entry-relative image links shown in the rich editor are read from the project's files. */
function resolveImage(src: string): string {
  const e = store.entry; const path = e ? entryAssetPath(e.path, src) : null;
  return path ? daemon.api.rawFileUrl(props.projectId, path) : src;
}
async function resume(change: Change) { try { await store.resumeSave(change.branch); } catch { /* recorded on the store */ } }
const differences = computed(() => {
  const now = store.current; if (!now) return [];
  const mine = store.draftFields, theirs = now.fields ?? {};
  const rows = [...new Set([...Object.keys(mine), ...Object.keys(theirs)])]
    .filter(k => !sameValue(mine[k], theirs[k])).map(k => ({ name: k, draft: mine[k], current: theirs[k] }));
  if (now.kind !== "json" && store.draftBody !== now.body) rows.push({ name: "body", draft: store.draftBody, current: now.body });
  return rows;
});
const show = (v: unknown) => v === undefined ? "(not set)" : typeof v === "string" ? v : JSON.stringify(v);
function beforeUnload(event: BeforeUnloadEvent) { if(store.dirty) { event.preventDefault(); event.returnValue = ""; } }
onMounted(() => window.addEventListener("beforeunload", beforeUnload));
onBeforeUnmount(() => window.removeEventListener("beforeunload", beforeUnload));
</script>
<template>
  <div class="flex h-full min-h-0" data-testid="content-panel">
    <aside class="w-48 shrink-0 overflow-auto border-r p-3">
      <h2 class="mb-3 text-sm font-medium">Collections</h2>
      <button v-for="collection in store.collections" :key="collection.name" data-testid="content-collection" class="block w-full rounded px-2 py-2 text-left text-sm hover:bg-muted" :class="{'bg-muted':store.selectedCollection === collection.name}" @click="store.selectCollection(collection.name)">
        {{ collection.name }}<span v-if="!collection.editable" class="block text-xs text-muted-foreground">Read only</span>
      </button>
    </aside>
    <aside v-if="store.selectedCollection" class="w-56 shrink-0 overflow-auto border-r p-3">
      <h2 class="mb-3 text-sm font-medium">Entries</h2>
      <template v-if="selected && !selected.singleton">
        <Button type="button" size="sm" variant="outline" class="mb-2 w-full" data-testid="content-create" :disabled="!!createBlocked" :title="createBlocked ?? undefined" @click="startCreate">New entry</Button>
        <p v-if="createBlocked" class="mb-2 text-xs text-muted-foreground" data-testid="content-create-reason">{{ createBlocked }}</p>
      </template>
      <button v-for="item in store.entries" :key="item.id" data-testid="content-entry" class="block w-full rounded px-2 py-2 text-left text-sm hover:bg-muted" @click="store.selectEntry(item.id)">{{ item.id }}</button>
      <p v-if="!store.entries.length && !store.loading" class="text-xs text-muted-foreground">No entries</p>
    </aside>
    <main class="min-w-0 flex-1 overflow-auto p-4">
      <p v-if="store.error" role="alert" class="text-sm text-destructive">{{ store.error }}</p>
      <p v-if="store.loading" class="text-sm text-muted-foreground">Loading content…</p>
      <p v-for="collection in store.collections.filter(c => c.reason && (!store.selectedCollection || c.name === store.selectedCollection))" :key="collection.name" class="mb-2 text-xs text-muted-foreground">{{ collection.name }}: {{ collection.reason }}</p>
      <section v-if="store.needsChange" role="alert" class="mb-3 rounded-md border p-3 text-sm" data-testid="content-needs-change">
        <p>Content cannot be edited on main. Create a change to {{ store.creation ? "create this entry" : "save your edits" }}.</p>
        <Button type="button" size="sm" class="mt-2" data-testid="content-create-change" @click="showNewChange = true">Create change</Button>
      </section>
      <template v-if="store.entry">
        <h2 class="font-medium">{{ store.entry.id }}</h2>
        <p class="mb-4 text-xs text-muted-foreground">{{ store.entry.path }} · {{ store.entry.branch || 'No branch' }}</p>
        <div v-if="canRemove" class="mb-4 flex gap-2">
          <Button type="button" variant="outline" size="sm" data-testid="content-rename" :disabled="store.dirty" @click="store.startLifecycle('rename')">Rename</Button>
          <Button type="button" variant="outline" size="sm" data-testid="content-delete" :disabled="store.dirty" @click="store.startLifecycle('delete')">Delete</Button>
          <span v-if="store.dirty" class="self-center text-xs text-muted-foreground" data-testid="content-lifecycle-blocked">Save or discard your changes first</span>
        </div>
        <p v-if="store.changedOnDisk" class="mb-3 text-sm" role="status">Content changed on disk. Your unsaved draft is preserved.</p>
        <form v-if="store.descriptor" data-testid="content-form" @submit.prevent="save">
          <FrontmatterForm :descriptor="store.descriptor" :model-value="store.draftFields" :errors="store.fieldErrors" @update:model-value="store.draftFields = $event as Record<string, unknown>" />
          <MarkdownBodyEditor v-if="store.entry.kind !== 'json'" :key="`${store.entry.collection}/${store.entry.id}`" :body="store.draftBody" :original="store.entry.body ?? ''" :kind="store.entry.kind" :rich="store.richBody" :resolve-image="resolveImage" @change="store.setBody($event.body)" />
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
          <Button type="submit" size="sm" data-testid="content-save" :disabled="!store.dirty || store.saving">{{ store.saving ? "Saving…" : "Save" }}</Button>
        </form>
        <template v-else>
          <pre data-testid="content-source" class="whitespace-pre-wrap break-words text-sm">{{ store.draftSource }}</pre>
        </template>
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
      </template>
      <p v-else-if="!store.loading" class="text-sm text-muted-foreground">Select a collection and entry to read its content.</p>
      <NewChangeDialog v-model:open="showNewChange" :project-id="props.projectId" @created="resume" />
      <Dialog v-model:open="createOpen">
        <DialogContent v-if="store.creation" data-testid="content-create-dialog">
          <form class="space-y-3" @submit.prevent="confirmCreate">
            <DialogHeader>
              <DialogTitle>New {{ store.creation.collection }} entry</DialogTitle>
              <DialogDescription>It starts from the collection's defaults; fill in the rest after it opens.</DialogDescription>
            </DialogHeader>
            <label class="block text-xs font-medium">Title
              <Input :model-value="store.creation.title" data-testid="content-create-title" class="mt-1" @update:model-value="setTitle(String($event))" />
            </label>
            <label class="block text-xs font-medium">Slug
              <Input :model-value="store.creation.slug" data-testid="content-create-slug" class="mt-1" placeholder="my-entry" @update:model-value="setSlug(String($event))" />
            </label>
            <p v-if="store.creation.error" role="alert" class="text-sm text-destructive" data-testid="content-create-error">{{ store.creation.error }}</p>
            <ul v-if="Object.keys(store.fieldErrors).length" class="text-xs text-destructive" data-testid="content-create-field-errors">
              <li v-for="(msg, path) in store.fieldErrors" :key="path">{{ path }}: {{ msg }}</li>
            </ul>
            <DialogFooter>
              <Button type="button" variant="outline" data-testid="content-create-cancel" @click="store.cancelCreate()">Cancel</Button>
              <Button type="submit" data-testid="content-create-confirm" :disabled="!canCreate">{{ store.creation.busy ? "Creating…" : "Create" }}</Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </main>
  </div>
</template>
