<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate } from "vue-router";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useDaemon } from "@/core/daemon";
import NewChangeDialog from "@/modules/changes/NewChangeDialog.vue";
import { useProjectsStore } from "@/modules/projects/store";
import type { Change } from "@/core/types";
import { useSettingsStore, type SettingsField } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useSettingsStore();
const projects = useProjectsStore();
const showNewChange = ref(false);

const FIELDS: { key: SettingsField; label: string }[] = [
  { key: "root", label: "Astro project folder" },
  { key: "installDir", label: "Install folder" },
  { key: "install", label: "Install command" },
  { key: "dev", label: "Dev command" },
];

/** Set once a save lands: the dev server restarts with the new settings, and its status is shown from then on. */
const saved = ref(false);
const message = ref<string | null>(null);
const devserver = computed(() => projects.projects.find(p => p.id === props.projectId)?.devserver);
const off = useDaemon().on("devserver.status", (e) => {
  if (e.projectId === props.projectId) message.value = e.state === "crashed" ? e.message ?? null : null;
});

watch(() => props.projectId, id => { saved.value = false; message.value = null; void store.open(id); }, { immediate: true });
onBeforeRouteLeave(() => store.canLeave());
onBeforeRouteUpdate(() => store.canLeave());

const fileErrors = computed(() => store.doc ? Object.keys(store.doc.errors).length > 0 : false);
const devMissingPort = computed(() => !!store.draft.dev && !store.draft.dev.includes("{port}"));
const devHint = computed(() => /\bastro\s+dev\b/.test(store.draft.dev) && !store.draft.dev.includes("--ignore-lock"));
function errorFor(key: SettingsField) {
  if (key === "dev" && devMissingPort.value) return "must contain {port}";
  return store.fieldErrors[key];
}

async function save() {
  if (devMissingPort.value) return;
  try { await store.save(); saved.value = true; } catch { /* recorded on the store; the draft stays */ }
}
async function resume(change: Change) {
  try { await store.resumeSave(change.branch); saved.value = true; } catch { /* recorded on the store */ }
}

function beforeUnload(event: BeforeUnloadEvent) { if (store.dirty) { event.preventDefault(); event.returnValue = ""; } }
onMounted(() => window.addEventListener("beforeunload", beforeUnload));
onBeforeUnmount(() => { window.removeEventListener("beforeunload", beforeUnload); off(); });
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="settings-panel">
    <template v-if="store.doc">
      <header class="relative z-10 flex h-11 shrink-0 items-center gap-1 bg-background px-2">
        <div class="pointer-events-none absolute inset-x-0 top-full h-6 bg-linear-to-b from-background to-transparent" aria-hidden="true" />
        <h2 class="truncate px-1 text-sm font-medium">orso.json</h2>
        <p class="truncate text-xs text-muted-foreground">{{ store.doc.branch }}</p>
        <div class="ml-auto flex shrink-0 items-center gap-2">
          <Button type="button" variant="ghost" size="sm" data-testid="settings-discard" :disabled="!store.dirty || store.saving" @click="store.discard()">Discard</Button>
          <Button type="button" variant="outline" size="sm" data-testid="settings-save" :disabled="!store.canSave || store.saving || devMissingPort" @click="save">{{ store.saving ? "Saving…" : "Save" }}</Button>
        </div>
      </header>
      <div class="min-h-0 flex-1 overflow-auto">
        <div class="mx-auto w-full max-w-2xl px-4 pt-4 pb-8">
          <p v-if="store.error" role="alert" class="mb-3 text-sm text-destructive" data-testid="settings-error">{{ store.error }}</p>
          <section v-if="store.needsChange" role="alert" class="mb-3 rounded-md border p-3 text-sm" data-testid="settings-needs-change">
            <p>Settings can't be edited on main. Create a change.</p>
            <Button type="button" size="sm" class="mt-2" data-testid="settings-create-change" @click="showNewChange = true">Create change</Button>
          </section>
          <section v-if="fileErrors" role="alert" class="mb-3 rounded-md border border-destructive p-3 text-sm" data-testid="settings-file-errors">
            <p>orso.json has errors — the dev server won't start until they're fixed.</p>
            <p v-if="store.doc.errors._file" class="mt-1 text-xs text-muted-foreground">{{ store.doc.errors._file }}</p>
          </section>
          <section v-if="store.changedOnDisk" role="status" class="mb-3 rounded-md border p-3 text-sm" data-testid="settings-changed-on-disk">
            <p>orso.json changed on disk. Your unsaved draft is preserved.</p>
            <Button type="button" variant="outline" size="sm" class="mt-2" data-testid="settings-reload" @click="store.reloadFromDisk()">Reload</Button>
          </section>
          <section v-if="store.conflict" role="alert" class="mb-3 rounded-md border border-destructive p-3 text-sm" data-testid="settings-conflict">
            <p>{{ store.conflict.message }}. Your edits are kept.</p>
            <Button type="button" variant="outline" size="sm" class="mt-2" data-testid="settings-reload" @click="store.reloadFromDisk()">Reload</Button>
          </section>

          <form class="grid gap-4" @submit.prevent="save">
            <div v-for="field in FIELDS" :key="field.key">
              <Label :for="`settings-${field.key}`" class="mb-1.5 text-xs">{{ field.label }}</Label>
              <Input
                :id="`settings-${field.key}`"
                :data-testid="`settings-${field.key}`"
                type="text"
                class="font-mono text-xs"
                :placeholder="store.doc.defaults[field.key]"
                :model-value="store.draft[field.key]"
                @update:model-value="store.setField(field.key, String($event))"
              />
              <p v-if="field.key === 'dev'" class="mt-1 text-xs text-muted-foreground">
                <code>{port}</code> is replaced with the preview port and <code>{host}</code> with 127.0.0.1. The command must contain <code>{port}</code>.
              </p>
              <p v-if="field.key === 'dev' && devHint" class="mt-1 text-xs text-muted-foreground" data-testid="settings-dev-hint">
                This looks like Astro without <code>--ignore-lock</code>; without it a restart can be blocked by the previous dev server.
              </p>
              <p v-if="errorFor(field.key)" role="alert" class="mt-1 text-xs text-destructive" :data-testid="`settings-error-${field.key}`">{{ errorFor(field.key) }}</p>
            </div>
          </form>

          <p v-if="saved && devserver" role="status" class="mt-6 text-sm text-muted-foreground" data-testid="settings-devserver">
            Dev server: {{ devserver }}<template v-if="devserver === 'crashed' && message"> — {{ message }}</template>
          </p>
        </div>
      </div>
      <NewChangeDialog v-model:open="showNewChange" :project-id="props.projectId" @created="resume" />
    </template>
    <p v-else-if="store.loading" class="p-4 text-sm text-muted-foreground">Loading settings…</p>
    <p v-else-if="store.error" role="alert" class="p-4 text-sm text-destructive" data-testid="settings-error">{{ store.error }}</p>
  </div>
</template>
