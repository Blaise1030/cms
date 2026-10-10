<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate } from "vue-router";
import { Button } from "@/components/ui/button";
import NewChangeDialog from "@/modules/changes/NewChangeDialog.vue";
import type { Change } from "@/core/types";
import FrontmatterForm from "./FrontmatterForm.vue";
import { sameValue } from "./draft";
import { useContentStore } from "./store";
const props = defineProps<{projectId: string}>();
const store = useContentStore();
watch(() => props.projectId, id => void store.open(id), {immediate:true});
onBeforeRouteLeave(() => store.canLeave());
onBeforeRouteUpdate(() => store.canLeave());
const showNewChange = ref(false);
async function save() { try { await store.saveDraft(); } catch { /* recorded on the store; the draft stays */ } }
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
      <button v-for="item in store.entries" :key="item.id" data-testid="content-entry" class="block w-full rounded px-2 py-2 text-left text-sm hover:bg-muted" @click="store.selectEntry(item.id)">{{ item.id }}</button>
      <p v-if="!store.entries.length && !store.loading" class="text-xs text-muted-foreground">No entries</p>
    </aside>
    <main class="min-w-0 flex-1 overflow-auto p-4">
      <p v-if="store.error" role="alert" class="text-sm text-destructive">{{ store.error }}</p>
      <p v-if="store.loading" class="text-sm text-muted-foreground">Loading content…</p>
      <p v-for="collection in store.collections.filter(c => c.reason && (!store.selectedCollection || c.name === store.selectedCollection))" :key="collection.name" class="mb-2 text-xs text-muted-foreground">{{ collection.name }}: {{ collection.reason }}</p>
      <template v-if="store.entry">
        <h2 class="font-medium">{{ store.entry.id }}</h2>
        <p class="mb-4 text-xs text-muted-foreground">{{ store.entry.path }} · {{ store.entry.branch || 'No branch' }}</p>
        <p v-if="store.changedOnDisk" class="mb-3 text-sm" role="status">Content changed on disk. Your unsaved draft is preserved.</p>
        <form v-if="store.descriptor" data-testid="content-form" @submit.prevent="save">
          <FrontmatterForm :descriptor="store.descriptor" :model-value="store.draftFields" :errors="store.fieldErrors" @update:model-value="store.draftFields = $event as Record<string, unknown>" />
          <label v-if="store.entry.kind !== 'json'" class="mb-3 block text-xs font-medium">Body
            <textarea v-model="store.draftBody" data-testid="content-body" rows="12" class="mt-1 w-full rounded-md border bg-background px-2 py-1 font-mono text-sm" />
          </label>
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
          <section v-if="store.needsChange" role="alert" class="mb-3 rounded-md border p-3 text-sm" data-testid="content-needs-change">
            <p>Content cannot be edited on main. Create a change to save your edits.</p>
            <Button type="button" size="sm" class="mt-2" data-testid="content-create-change" @click="showNewChange = true">Create change</Button>
          </section>
          <Button type="submit" size="sm" data-testid="content-save" :disabled="!store.dirty || store.saving">{{ store.saving ? "Saving…" : "Save" }}</Button>
        </form>
        <template v-else>
          <pre data-testid="content-source" class="whitespace-pre-wrap break-words text-sm">{{ store.draftSource }}</pre>
        </template>
        <NewChangeDialog v-model:open="showNewChange" :project-id="props.projectId" @created="resume" />
      </template>
      <p v-else-if="!store.loading" class="text-sm text-muted-foreground">Select a collection and entry to read its content.</p>
    </main>
  </div>
</template>
