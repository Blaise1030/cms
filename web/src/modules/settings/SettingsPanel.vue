<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate } from "vue-router";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UFormField from "@nuxt/ui/components/FormField.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UInputTags from "@nuxt/ui/components/InputTags.vue";
import USelect from "@nuxt/ui/components/Select.vue";
import { useDaemon } from "@/core/daemon";
import NewChangeDialog from "@/modules/changes/NewChangeDialog.vue";
import { useProjectsStore } from "@/modules/projects/store";
import type { Change } from "@/core/types";
import { PUBLISH_FIELDS, useSettingsStore, type SettingsField, type TextField } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useSettingsStore();
const projects = useProjectsStore();
const showNewChange = ref(false);

const FIELDS: { key: TextField; label: string }[] = [
  { key: "root", label: "Astro project folder" },
  { key: "installDir", label: "Install folder" },
  { key: "install", label: "Install command" },
  { key: "dev", label: "Dev command" },
];
const MERGE_METHODS = [
  { label: "Squash (recommended)", value: "squash" },
  { label: "Merge commit", value: "merge" },
];

/** An unset method shows the default; picking the default keeps it unset, so the file only changes when it must. */
const mergeMethod = computed(() => store.draft.mergeMethod || store.doc?.defaults.mergeMethod);
function setMergeMethod(value: string) {
  const unset = typeof store.doc?.settings.mergeMethod !== "string" || store.doc.settings.mergeMethod === "";
  store.setField("mergeMethod", unset && value === store.doc?.defaults.mergeMethod ? "" : value);
}
/** No labels typed means the default ones; `[]` written in the file (labelling off) reads as "No labels". */
function setPrLabels(labels: string[]) { store.setField("prLabels", labels.length ? labels : null); }
const prLabelsPlaceholder = computed(() => store.draft.prLabels === null && store.doc?.defaults.prLabels.length
  ? `${store.doc.defaults.prLabels.join(", ")} (default)`
  : "No labels");

/** Set once a save lands: the dev server restarts with the new settings, and its status is shown from then on. */
const saved = ref(false);
/** A save just landed: the restart it causes has not reported yet, so the last known state is stale. */
const restarting = ref(false);
const message = ref<string | null>(null);
const devserver = computed(() => restarting.value ? "starting" : projects.projects.find(p => p.id === props.projectId)?.devserver);
const off = useDaemon().on("devserver.status", (e) => {
  if (e.projectId !== props.projectId) return;
  restarting.value = false;
  message.value = e.state === "crashed" ? e.message ?? null : null;
});
function landed() { saved.value = true; restarting.value = true; message.value = null; }

watch(() => props.projectId, id => { saved.value = false; restarting.value = false; message.value = null; void store.open(id); }, { immediate: true });
onBeforeRouteLeave(() => store.canLeave());
onBeforeRouteUpdate(() => store.canLeave());

/** Errors that stop the dev server; the publishing fields fall back to their defaults instead. */
const fileErrors = computed(() => store.doc ? Object.keys(store.doc.errors).some(key => !PUBLISH_FIELDS.includes(key)) : false);
const devMissingPort = computed(() => !!store.draft.dev && !store.draft.dev.includes("{port}"));
const devHint = computed(() => /\bastro\s+dev\b/.test(store.draft.dev) && !store.draft.dev.includes("--ignore-lock"));
function errorFor(key: SettingsField) {
  if (key === "dev" && devMissingPort.value) return "must contain {port}";
  return store.fieldErrors[key];
}

async function save() {
  if (devMissingPort.value) return;
  try { await store.save(); landed(); } catch { /* recorded on the store; the draft stays */ }
}
async function resume(change: Change) {
  try { await store.resumeSave(change.branch); landed(); } catch { /* recorded on the store */ }
}

function beforeUnload(event: BeforeUnloadEvent) { if (store.dirty) { event.preventDefault(); event.returnValue = ""; } }
onMounted(() => window.addEventListener("beforeunload", beforeUnload));
onBeforeUnmount(() => { window.removeEventListener("beforeunload", beforeUnload); off(); });
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="settings-panel">
    <template v-if="store.doc">
      <header class="bg-background relative z-10 flex h-11 shrink-0 items-center gap-1 px-2">
        <div class="from-background pointer-events-none absolute inset-x-0 top-full h-6 bg-linear-to-b to-transparent" aria-hidden="true" />
        <h2 class="text-highlighted truncate px-1 text-sm font-medium">orso.json</h2>
        <p class="text-muted truncate text-xs">{{ store.doc.branch }}</p>
        <div class="ml-auto flex shrink-0 items-center gap-2">
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            label="Discard"
            data-testid="settings-discard"
            :disabled="!store.dirty || store.saving"
            @click="store.discard()"
          />
          <UButton
            color="neutral"
            variant="outline"
            size="sm"
            :label="store.saving ? 'Saving…' : 'Save'"
            data-testid="settings-save"
            :disabled="!store.canSave || store.saving || devMissingPort"
            @click="save"
          />
        </div>
      </header>
      <div class="min-h-0 flex-1 overflow-auto">
        <div class="mx-auto flex w-full max-w-2xl flex-col gap-3 px-4 pt-4 pb-8">
          <p v-if="store.error" role="alert" class="text-error text-sm" data-testid="settings-error">{{ store.error }}</p>
          <UAlert
            v-if="store.needsChange"
            role="alert"
            color="neutral"
            variant="outline"
            icon="i-hugeicons-git-branch"
            description="Settings can't be edited on the live site. Create a change."
            data-testid="settings-needs-change"
          >
            <template #actions>
              <UButton size="sm" label="Create change" data-testid="settings-create-change" @click="showNewChange = true" />
            </template>
          </UAlert>
          <UAlert
            v-if="fileErrors"
            role="alert"
            color="error"
            variant="subtle"
            icon="i-hugeicons-alert-circle"
            title="orso.json has errors — the dev server won't start until they're fixed."
            :description="store.doc.errors._file"
            data-testid="settings-file-errors"
          />
          <UAlert
            v-if="store.changedOnDisk"
            role="status"
            color="neutral"
            variant="outline"
            icon="i-hugeicons-git-compare"
            description="orso.json changed on disk. Your unsaved draft is preserved."
            data-testid="settings-changed-on-disk"
          >
            <template #actions>
              <UButton color="neutral" variant="outline" size="sm" label="Reload" data-testid="settings-reload" @click="store.reloadFromDisk()" />
            </template>
          </UAlert>
          <UAlert
            v-if="store.conflict"
            role="alert"
            color="error"
            variant="subtle"
            icon="i-hugeicons-alert-circle"
            :description="`${store.conflict.message}. Your edits are kept.`"
            data-testid="settings-conflict"
          >
            <template #actions>
              <UButton color="neutral" variant="outline" size="sm" label="Reload" data-testid="settings-reload" @click="store.reloadFromDisk()" />
            </template>
          </UAlert>

          <form class="mt-1 grid gap-4" @submit.prevent="save">
            <UFormField
              v-for="field in FIELDS"
              :key="field.key"
              :label="field.label"
              :name="field.key"
              size="sm"
            >
              <UInput
                :id="`settings-${field.key}`"
                :data-testid="`settings-${field.key}`"
                type="text"
                class="w-full"
                :ui="{ base: 'font-mono' }"
                :placeholder="store.doc.defaults[field.key]"
                :model-value="store.draft[field.key]"
                @update:model-value="store.setField(field.key, String($event))"
              />
              <template v-if="field.key === 'dev'" #help>
                <span class="block">
                  <code>{port}</code> is replaced with the preview port and <code>{host}</code> with 127.0.0.1. The command must contain <code>{port}</code>.
                </span>
                <span v-if="devHint" class="mt-1 block" data-testid="settings-dev-hint">
                  This looks like Astro without <code>--ignore-lock</code>; without it a restart can be blocked by the previous dev server.
                </span>
              </template>
              <p v-if="errorFor(field.key)" role="alert" class="text-error mt-1 text-xs" :data-testid="`settings-error-${field.key}`">{{ errorFor(field.key) }}</p>
            </UFormField>

            <UFormField label="Merge method" name="mergeMethod" size="sm">
              <USelect
                id="settings-mergeMethod"
                data-testid="settings-mergeMethod"
                class="w-full"
                :items="MERGE_METHODS"
                :model-value="mergeMethod"
                @update:model-value="setMergeMethod(String($event))"
              />
              <template #help>
                <span class="block">How Publish adds a change to the live site. Squash adds it as one commit; a merge commit keeps every checkpoint.</span>
              </template>
              <p v-if="errorFor('mergeMethod')" role="alert" class="text-error mt-1 text-xs" data-testid="settings-error-mergeMethod">{{ errorFor("mergeMethod") }}</p>
            </UFormField>

            <UFormField label="Pull request labels" name="prLabels" size="sm">
              <UInputTags
                id="settings-prLabels"
                data-testid="settings-prLabels"
                class="w-full"
                add-on-blur
                :placeholder="prLabelsPlaceholder"
                :model-value="store.draft.prLabels ?? []"
                @update:model-value="setPrLabels($event.map(String))"
              />
              <template #help>
                <span class="block">Added to the pull requests orso opens, so reviewers can find them. Type a label and press Enter.</span>
              </template>
              <p v-if="errorFor('prLabels')" role="alert" class="text-error mt-1 text-xs" data-testid="settings-error-prLabels">{{ errorFor("prLabels") }}</p>
            </UFormField>
          </form>

          <p v-if="saved && devserver" role="status" class="text-muted mt-3 text-sm" data-testid="settings-devserver">
            Dev server: {{ devserver }}<template v-if="devserver === 'crashed' && message"> — {{ message }}</template>
          </p>
        </div>
      </div>
      <NewChangeDialog v-model:open="showNewChange" :project-id="props.projectId" @created="resume" />
    </template>
    <p v-else-if="store.loading" class="text-muted p-4 text-sm">Loading settings…</p>
    <p v-else-if="store.error" role="alert" class="text-error p-4 text-sm" data-testid="settings-error">{{ store.error }}</p>
  </div>
</template>
