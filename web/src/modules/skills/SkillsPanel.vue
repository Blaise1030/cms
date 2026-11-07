<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Sparkles } from "@lucide/vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate } from "vue-router";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Sidebar, SidebarContent, SidebarGroup, SidebarGroupContent, SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import NewChangeDialog from "@/modules/changes/NewChangeDialog.vue";
import type { Change, SkillMeta } from "@/core/types";
import { useSkillsStore } from "./store";
import InstallSkillDialog from "./InstallSkillDialog.vue";

// Tiptap and CodeMirror load with the first skill, not with the app shell.
const MarkdownBodyEditor = defineAsyncComponent(() => import("@/modules/content/MarkdownBodyEditor.vue"));

const props = defineProps<{ projectId: string }>();
const store = useSkillsStore();
const showNewChange = ref(false);
/** The install dialog is opened from the header and the empty/no-selection states. */
const installOpen = ref(false);

watch(() => props.projectId, id => void store.open(id), { immediate: true });
onBeforeRouteLeave(() => store.canLeave());
onBeforeRouteUpdate(() => store.canLeave());

async function save() { try { await store.save(); } catch { /* recorded on the store; the draft stays */ } }
async function uninstall() {
  if (store.selected?.scope === "global" && !window.confirm("Remove this global skill? This cannot be undone.")) return;
  try { await store.uninstall(); } catch { /* recorded on the store */ }
}
async function duplicateToProject() {
  const sel = store.selected;
  if (!sel || sel.scope !== "global") return;
  try { await store.install("project", { copy: { id: sel.id, fromScope: "global" } }); } catch { /* recorded on the store; Create change is the retry path */ }
}
async function resume(change: Change) { try { await store.resumeSave(change.branch); } catch { /* recorded on the store */ } }

function beforeUnload(event: BeforeUnloadEvent) { if (store.dirty) { event.preventDefault(); event.returnValue = ""; } }
onMounted(() => window.addEventListener("beforeunload", beforeUnload));
onBeforeUnmount(() => window.removeEventListener("beforeunload", beforeUnload));

const isSelected = (m: SkillMeta) => store.selected?.id === m.id && store.selected?.scope === m.scope;
/** Project saves are change-gated; global saves only need a dirty draft. */
const saveDisabled = computed(() => !store.dirty || store.saving || (store.doc?.scope === "project" && store.needsChange));

function select(m: SkillMeta) { void store.select(m.id, m.scope); }
</script>

<template>
  <div class="h-full min-h-0" data-testid="skills-panel">
    <SidebarProvider class="h-full min-h-0 overflow-hidden" :shortcut="false">
      <Sidebar data-testid="skills-sidebar" collapsible="none" class="h-full min-h-0 w-64 min-w-0 shrink-0 bg-transparent">
        <SidebarContent class="overflow-hidden">
          <SidebarGroup class="min-h-0 py-2">
            <SidebarGroupContent>
              <nav data-testid="skills-list" aria-label="Installed skills" class="flex flex-col gap-0.5">
                <button
                  v-for="m in store.list"
                  :key="`${m.id}:${m.scope}`"
                  type="button"
                  class="group flex min-w-0 flex-col gap-0.5 rounded-md px-2 py-1.5 text-left outline-none hover:bg-accent/60 focus-visible:bg-accent"
                  :class="isSelected(m) ? 'bg-accent text-accent-foreground' : ''"
                  :aria-current="isSelected(m) ? 'page' : undefined"
                  data-testid="skills-sidebar-row"
                  :data-skill-id="m.id"
                  :data-skill-scope="m.scope"
                  @click="select(m)"
                >
                  <span class="flex min-w-0 items-center gap-1.5">
                    <span class="truncate text-sm font-medium">{{ m.name }}</span>
                    <Badge variant="outline" class="shrink-0">{{ m.scope }}</Badge>
                  </span>
                  <span class="truncate text-xs text-muted-foreground">{{ m.description }}</span>
                </button>
                <p v-if="!store.list.length" class="px-2 py-1.5 text-xs text-muted-foreground">None installed</p>
              </nav>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
      </Sidebar>

      <SidebarInset class="flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-background">
        <p v-if="store.error" role="alert" class="shrink-0 px-4 pt-3 pb-1 text-sm text-destructive" data-testid="skills-error">{{ store.error }}</p>
        <section v-if="store.needsChange" role="alert" class="mx-4 mt-3 shrink-0 rounded-md border p-3 text-sm" data-testid="skills-needs-change">
          <p>Create a change to edit, install, or remove project skills.</p>
          <Button type="button" size="sm" class="mt-2" data-testid="skills-create-change" @click="showNewChange = true">Create change</Button>
        </section>
        <section v-if="store.invalid" role="alert" class="flex min-h-0 flex-1 flex-col overflow-auto p-4 text-sm" data-testid="skills-invalid">
          <p class="rounded-md border border-destructive p-3">This skill has invalid frontmatter. Fix it, or remove it.</p>
          <p class="mt-2 text-xs text-muted-foreground">{{ store.invalid }}</p>
          <pre v-if="store.invalidSource" class="mt-2 max-h-96 overflow-auto rounded-md bg-muted p-3 font-mono text-xs whitespace-pre-wrap" data-testid="skills-invalid-source">{{ store.invalidSource }}</pre>
          <div class="mt-3">
            <Button type="button" variant="destructive" size="sm" data-testid="skills-uninstall" @click="uninstall">Uninstall</Button>
          </div>
        </section>

        <template v-else-if="store.doc">
          <header class="relative z-10 flex h-11 shrink-0 items-center gap-1 bg-background px-2">
            <div class="pointer-events-none absolute inset-x-0 top-full h-6 bg-linear-to-b from-background to-transparent" aria-hidden="true" />
            <h2 class="truncate px-1 text-sm font-medium">{{ store.doc.name }}</h2>
            <p class="truncate text-xs text-muted-foreground" data-testid="skills-path" :title="store.doc.path">{{ store.doc.path }}</p>
            <div class="ml-auto flex shrink-0 items-center gap-2">
              <Button type="button" variant="outline" size="sm" data-testid="skills-install" @click="installOpen = true">Install…</Button>
              <Button v-if="store.selected?.scope === 'global'" type="button" variant="secondary" size="sm" data-testid="skills-duplicate" @click="duplicateToProject">Duplicate to project</Button>
              <Button type="button" variant="ghost" size="sm" data-testid="skills-discard" :disabled="!store.dirty || store.saving" @click="store.discard()">Discard</Button>
              <Button type="button" size="sm" data-testid="skills-save" :disabled="saveDisabled" @click="save">{{ store.saving ? "Saving…" : "Save" }}</Button>
              <Button type="button" variant="destructive" size="sm" data-testid="skills-uninstall" @click="uninstall">Uninstall</Button>
            </div>
          </header>
          <div class="min-h-0 flex-1 overflow-auto [container-type:size]">
            <div class="px-4">
              <section v-if="store.changedOnDisk" role="status" class="mb-3 rounded-md border p-3 text-sm" data-testid="skills-changed-on-disk">
                <p>The skill changed on disk. Your unsaved draft is preserved.</p>
                <Button type="button" variant="outline" size="sm" class="mt-2" data-testid="skills-reload" @click="store.reloadFromDisk()">Reload</Button>
              </section>
              <section v-if="store.conflict" role="alert" class="mb-3 rounded-md border border-destructive p-3 text-sm" data-testid="skills-conflict">
                <p>{{ store.conflict.message }}. Your edits are kept.</p>
                <Button type="button" variant="outline" size="sm" class="mt-2" data-testid="skills-reload" @click="store.reloadFromDisk()">Reload</Button>
              </section>
            </div>
            <div class="mx-auto w-full max-w-3xl px-4 pt-4 pb-8" data-testid="skills-editor">
              <div class="space-y-1">
                <Label for="skills-name" class="text-xs font-medium">Name</Label>
                <Input id="skills-name" :model-value="store.draftName" data-testid="skills-name" class="h-8" @update:model-value="store.setName(String($event))" />
              </div>
              <div class="mt-4 space-y-1">
                <Label for="skills-description" class="text-xs font-medium">Description</Label>
                <Input id="skills-description" :model-value="store.draftDescription" data-testid="skills-description" class="h-8" @update:model-value="store.setDescription(String($event))" />
              </div>
              <MarkdownBodyEditor :body="store.draftBody" :original="store.doc.body" kind="markdown" :rich="true" @change="store.setBody($event.body)" />
            </div>
          </div>
        </template>

        <div v-else-if="!store.loading && !store.list.length" class="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 px-4 pb-11 text-center text-sm text-muted-foreground" data-testid="skills-empty">
          <Sparkles class="size-8 opacity-50" aria-hidden="true" />
          <p class="font-medium text-foreground">No skills installed</p>
          <Button type="button" data-testid="skills-install" @click="installOpen = true">Install…</Button>
        </div>

        <p v-else-if="store.loading" class="shrink-0 p-4 text-sm text-muted-foreground">Loading skills…</p>

        <div v-else class="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 px-4 pb-11 text-center text-sm text-muted-foreground" data-testid="skills-no-selection">
          <Sparkles class="size-8 opacity-50" aria-hidden="true" />
          <p>Select a skill or install one.</p>
          <Button type="button" data-testid="skills-install" @click="installOpen = true">Install…</Button>
        </div>

        <NewChangeDialog v-model:open="showNewChange" :project-id="props.projectId" @created="resume" />
      </SidebarInset>
    </SidebarProvider>

    <InstallSkillDialog v-model:open="installOpen" />
  </div>
</template>
