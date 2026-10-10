<script setup lang="ts">
import { onBeforeUnmount, onMounted, watch } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate } from "vue-router";
import { useContentStore } from "./store";
const props = defineProps<{projectId: string}>();
const store = useContentStore();
watch(() => props.projectId, id => void store.open(id), {immediate:true});
onBeforeRouteLeave(() => store.canLeave());
onBeforeRouteUpdate(() => store.canLeave());
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
        <pre data-testid="content-source" class="whitespace-pre-wrap break-words text-sm">{{ store.draftSource }}</pre>
      </template>
      <p v-else-if="!store.loading" class="text-sm text-muted-foreground">Select a collection and entry to read its content.</p>
    </main>
  </div>
</template>
