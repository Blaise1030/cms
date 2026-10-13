<script setup lang="ts">
import { defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Palette } from "@lucide/vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate } from "vue-router";
import { Button } from "@/components/ui/button";
import NewChangeDialog from "@/modules/changes/NewChangeDialog.vue";
import type { Change } from "@/core/types";
import { askAgentToApplyDesign } from "./agentRequests";
import DesignTokensForm from "./DesignTokensForm.vue";
import { useDesignStore } from "./store";

// Tiptap and CodeMirror load with the first design, not with the app shell.
const MarkdownBodyEditor = defineAsyncComponent(() => import("@/modules/content/MarkdownBodyEditor.vue"));
const props = defineProps<{ projectId: string }>();
const store = useDesignStore();
const showNewChange = ref(false);

watch(() => props.projectId, id => void store.open(id), { immediate: true });
onBeforeRouteLeave(() => store.canLeave());
onBeforeRouteUpdate(() => store.canLeave());

async function save() { try { await store.save(); } catch { /* recorded on the store; the draft stays */ } }
async function resume(change: Change) { try { await store.resumeSave(change.branch); } catch { /* recorded on the store */ } }

function beforeUnload(event: BeforeUnloadEvent) { if (store.dirty) { event.preventDefault(); event.returnValue = ""; } }
onMounted(() => window.addEventListener("beforeunload", beforeUnload));
onBeforeUnmount(() => window.removeEventListener("beforeunload", beforeUnload));
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="design-panel">
    <section v-if="store.invalid" role="alert" class="m-4 rounded-md border border-destructive p-3 text-sm" data-testid="design-invalid">
      <p>DESIGN.md has invalid frontmatter. Ask the agent to fix it.</p>
      <p class="mt-1 text-xs text-muted-foreground">{{ store.invalid }}</p>
      <pre v-if="store.invalidSource" class="mt-2 max-h-96 overflow-auto rounded-md bg-muted p-3 font-mono text-xs whitespace-pre-wrap" data-testid="design-invalid-source">{{ store.invalidSource }}</pre>
    </section>
    <div v-else-if="store.doc && !store.doc.exists && !store.dirty" class="flex h-full flex-col items-center justify-center gap-3 px-4 pb-11 text-center text-sm text-muted-foreground">
      <Palette class="size-8 opacity-50" aria-hidden="true" />
      <p>This project has no DESIGN.md yet.</p>
      <Button type="button" data-testid="design-create" @click="store.create()">Create design</Button>
    </div>
    <template v-else-if="store.doc">
      <header class="relative z-10 flex h-11 shrink-0 items-center gap-1 bg-background px-2">
        <div class="pointer-events-none absolute inset-x-0 top-full h-6 bg-linear-to-b from-background to-transparent" aria-hidden="true" />
        <h2 class="truncate px-1 text-sm font-medium">DESIGN.md</h2>
        <p class="truncate text-xs text-muted-foreground">{{ store.doc.branch }}</p>
        <div class="ml-auto flex shrink-0 items-center gap-1">
          <Button type="button" variant="ghost" size="sm" data-testid="design-discard" :disabled="!store.dirty || store.saving" @click="store.discard()">Discard</Button>
          <Button type="button" variant="outline" size="sm" data-testid="design-save" :disabled="!store.dirty || store.saving" @click="save">{{ store.saving ? "Saving…" : "Save" }}</Button>
          <Button type="button" size="sm" data-testid="design-apply" :disabled="store.dirty || !store.doc.exists" :title="store.dirty ? 'Save your changes first' : undefined" @click="askAgentToApplyDesign()">Apply to site</Button>
        </div>
      </header>
      <div class="min-h-0 flex-1 overflow-auto [container-type:size]">
        <div class="px-4">
          <p v-if="store.error" role="alert" class="text-sm text-destructive" data-testid="design-error">{{ store.error }}</p>
          <section v-if="store.needsChange" role="alert" class="mb-3 rounded-md border p-3 text-sm" data-testid="design-needs-change">
            <p>The design cannot be edited on main. Create a change to save your edits.</p>
            <Button type="button" size="sm" class="mt-2" data-testid="design-create-change" @click="showNewChange = true">Create change</Button>
          </section>
          <section v-if="store.changedOnDisk" role="status" class="mb-3 rounded-md border p-3 text-sm" data-testid="design-changed-on-disk">
            <p>DESIGN.md changed on disk. Your unsaved draft is preserved.</p>
            <Button type="button" variant="outline" size="sm" class="mt-2" data-testid="design-reload" @click="store.reloadFromDisk()">Reload</Button>
          </section>
          <section v-if="store.conflict" role="alert" class="mb-3 rounded-md border border-destructive p-3 text-sm" data-testid="design-conflict">
            <p>{{ store.conflict.message }}. Your edits are kept.</p>
            <Button type="button" variant="outline" size="sm" class="mt-2" data-testid="design-reload" @click="store.reloadFromDisk()">Reload</Button>
          </section>
        </div>
        <div class="@container flex min-h-full">
          <div class="flex min-h-full w-full flex-col @3xl:flex-row">
            <aside class="min-w-0 px-4 pt-6 pb-4 @3xl:sticky @3xl:top-0 @3xl:order-2 @3xl:max-h-[100cqh] @3xl:w-80 @3xl:shrink-0 @3xl:self-start @3xl:overflow-y-auto" data-testid="design-side">
              <DesignTokensForm :model-value="store.draftTokens" @update:model-value="store.setTokens" />
            </aside>
            <div class="min-w-0 flex-1 px-4 pt-4 pb-8 @3xl:order-1 @3xl:px-6">
              <div class="mx-auto w-full max-w-3xl">
                <MarkdownBodyEditor :body="store.draftBody" :original="store.doc.body" kind="markdown" :rich="true" @change="store.setBody($event.body)" />
              </div>
            </div>
          </div>
        </div>
      </div>
      <NewChangeDialog v-model:open="showNewChange" :project-id="props.projectId" @created="resume" />
    </template>
    <p v-else-if="store.loading" class="p-4 text-sm text-muted-foreground">Loading design…</p>
    <p v-else-if="store.error" role="alert" class="p-4 text-sm text-destructive" data-testid="design-error">{{ store.error }}</p>
  </div>
</template>
