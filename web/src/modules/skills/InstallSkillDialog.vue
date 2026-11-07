<script setup lang="ts">
import { computed, ref, watch, type Component } from "vue";
import { Check, Download, Palette, Search, Sparkles } from "@lucide/vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { CatalogItem, SkillScope, SkillSource } from "@/core/types";
import { useSkillsStore } from "./store";

const open = defineModel<boolean>("open", { default: false });
const store = useSkillsStore();

const segment = ref<"catalog" | "import">("catalog");
const scope = ref<SkillScope>("project");
const query = ref("");
const skillMd = ref("");
const fileName = ref("");
const blankName = ref("");
const blankDescription = ref("");
const blankBody = ref("");
/** "New skill" opens a full editor instead of the catalog/import views. */
const creatingSkill = ref(false);

const error = computed(() => store.error ?? store.conflict?.message ?? null);

/** Each catalog skill gets a colorful tile, like an app icon. */
const CATALOG_STYLE: Record<string, { icon: Component; color: string }> = {
  seo: { icon: Search, color: "bg-teal-600" },
  design: { icon: Palette, color: "bg-blue-600" },
};
const catalogStyle = (id: string) =>
  CATALOG_STYLE[id] ?? { icon: Sparkles, color: "bg-slate-600" };

const visibleCatalog = computed(() => {
  const needle = query.value.trim().toLowerCase();
  if (!needle) return store.catalog;
  return store.catalog.filter(item =>
    `${item.name} ${item.description}`.toLowerCase().includes(needle),
  );
});

watch(open, (isOpen) => {
  if (!isOpen) return;
  // A fresh open starts clean: a stale install error/conflict must not linger.
  store.error = null;
  store.conflict = null;
  segment.value = "catalog";
  scope.value = "project";
  query.value = "";
  skillMd.value = "";
  fileName.value = "";
  blankName.value = "";
  blankDescription.value = "";
  blankBody.value = "";
  creatingSkill.value = false;
}, { immediate: true });

function installedInScope(item: CatalogItem) {
  return scope.value === "project" ? item.installedProject : item.installedGlobal;
}

/** Run an install; on success the store already reloads and selects, so just close. */
async function install(source: SkillSource) {
  try {
    await store.install(scope.value, source);
    open.value = false;
  } catch (err) {
    // A project install refused on main is handled by the panel's Create-change banner.
    if (store.needsChange) open.value = false;
  }
}

/** "New skill" opens the inline skill editor. */
function newSkill() {
  blankName.value = "";
  blankDescription.value = "";
  blankBody.value = "";
  creatingSkill.value = true;
}

/** Create the blank skill, then save the instructions the writer typed. */
async function createSkill() {
  try {
    await store.install(scope.value, {
      blank: { name: blankName.value.trim(), description: blankDescription.value.trim() },
    });
    if (blankBody.value.trim()) {
      store.setBody(blankBody.value);
      await store.save();
    }
    creatingSkill.value = false;
    open.value = false;
  } catch (err) {
    if (store.needsChange) open.value = false;
  }
}

function onFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  fileName.value = file.name;
  const reader = new FileReader();
  reader.onload = () => { skillMd.value = typeof reader.result === "string" ? reader.result : ""; };
  reader.onerror = () => { skillMd.value = ""; fileName.value = ""; };
  reader.readAsText(file);
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent
      :show-close-button="false"
      class="flex max-h-[94vh] min-h-[min(680px,88vh)] flex-col gap-4 overflow-y-auto p-5 sm:max-w-5xl sm:p-6"
      data-testid="install-skill-dialog"
    >
      <DialogHeader class="sr-only">
        <DialogTitle>Install skill</DialogTitle>
        <DialogDescription>Add a built-in skill, import a SKILL.md, or create a blank skill.</DialogDescription>
      </DialogHeader>

      <p v-if="error" role="alert" class="text-destructive text-sm" data-testid="install-error">
        {{ error }}
      </p>

      <template v-if="creatingSkill">
        <div class="flex min-h-0 flex-1 flex-col gap-4">
          <h3 class="text-base font-semibold">New skill</h3>
          <div class="grid min-h-0 flex-1 gap-4 sm:grid-cols-[minmax(0,18rem)_minmax(0,1fr)]">
            <div class="space-y-4">
              <div class="space-y-1.5">
                <Label for="install-blank-name" class="text-xs font-medium">Name</Label>
                <Input id="install-blank-name" v-model="blankName" data-testid="install-blank-name" />
              </div>
              <div class="space-y-1.5">
                <Label for="install-blank-description" class="text-xs font-medium">Description</Label>
                <Textarea
                  id="install-blank-description"
                  v-model="blankDescription"
                  data-testid="install-blank-description"
                  rows="3"
                  class="min-h-20 resize-y"
                />
              </div>
            </div>
            <div class="flex min-h-0 flex-col gap-1.5">
              <Label for="install-blank-body" class="text-xs font-medium">Instructions</Label>
              <Textarea
                id="install-blank-body"
                v-model="blankBody"
                data-testid="install-blank-body"
                placeholder="When should the agent use this skill, and what should it do?"
                class="min-h-64 flex-1 resize-none font-mono text-sm"
              />
            </div>
          </div>
          <div class="flex justify-end gap-2 border-t pt-4">
            <Button type="button" variant="ghost" @click="creatingSkill = false">Cancel</Button>
            <Button
              type="button"
              :disabled="!blankName.trim() || !blankDescription.trim() || store.saving"
              data-testid="install-blank-create"
              @click="createSkill"
            >
              {{ store.saving ? "Creating…" : "Create skill" }}
            </Button>
          </div>
        </div>
      </template>

      <Tabs v-else v-model="segment" class="min-w-0">
        <div class="flex flex-wrap items-center gap-3">
          <TabsList class="h-9 gap-2 bg-transparent p-0">
            <TabsTrigger
              value="catalog"
              class="h-9 rounded-full border-0 px-4 text-[15px] data-[state=active]:bg-secondary data-[state=active]:text-secondary-foreground dark:data-[state=active]:bg-secondary data-[state=inactive]:bg-transparent data-[state=inactive]:text-muted-foreground data-[state=inactive]:hover:bg-accent data-[state=inactive]:hover:text-accent-foreground"
              data-testid="install-segment-catalog"
            >
              Catalog
            </TabsTrigger>
            <TabsTrigger
              value="import"
              class="h-9 rounded-full border-0 px-4 text-[15px] data-[state=active]:bg-secondary data-[state=active]:text-secondary-foreground dark:data-[state=active]:bg-secondary data-[state=inactive]:bg-transparent data-[state=inactive]:text-muted-foreground data-[state=inactive]:hover:bg-accent data-[state=inactive]:hover:text-accent-foreground"
              data-testid="install-segment-import"
            >
              Import
            </TabsTrigger>
          </TabsList>

          <div class="ml-auto flex w-full flex-wrap items-center justify-end gap-3 sm:w-auto">
            <InputGroup class="h-9 w-full rounded-full sm:w-64">
              <InputGroupAddon align="inline-start">
                <Search class="size-4 opacity-50" aria-hidden="true" />
              </InputGroupAddon>
              <InputGroupInput
                v-model="query"
                placeholder="Search…"
                aria-label="Search skills"
                data-testid="install-search"
              />
            </InputGroup>
            <Button
              type="button"
              class="rounded-full px-5"
              data-testid="install-new"
              @click="newSkill"
            >
              New skill
            </Button>
          </div>
        </div>

        <TabsContent value="catalog" class="mt-4 min-w-0">
          <h3 class="px-2.5 text-sm font-semibold">Built-in</h3>
          <div class="mt-2 grid gap-x-8 gap-y-1 sm:grid-cols-2">
            <button
              v-for="item in visibleCatalog"
              :key="item.id"
              type="button"
              :disabled="installedInScope(item) || store.saving"
              :title="`Install ${item.name}`"
              data-testid="install-catalog-install"
              :data-skill-id="item.id"
              class="group hover:bg-muted/50 flex min-w-0 items-center gap-3 rounded-lg p-2.5 text-left transition-colors disabled:cursor-default disabled:hover:bg-transparent"
              @click="install({ catalog: { id: item.id } })"
            >
              <span
                class="flex size-10 shrink-0 items-center justify-center rounded-lg text-white"
                :class="catalogStyle(item.id).color"
              >
                <component :is="catalogStyle(item.id).icon" class="size-5" aria-hidden="true" />
              </span>
              <span class="min-w-0 flex-1">
                <span class="block truncate text-sm font-semibold">{{ item.name }}</span>
                <span class="text-muted-foreground block truncate text-xs">{{ item.description }}</span>
              </span>
              <Badge
                v-if="installedInScope(item)"
                variant="secondary"
                class="shrink-0 gap-1 text-xs"
              >
                <Check class="size-3.5" aria-hidden="true" /> Installed
              </Badge>
              <Download
                v-else
                class="size-4 shrink-0 opacity-0 transition-opacity group-hover:opacity-60"
                aria-hidden="true"
              />
            </button>
            <p
              v-if="!visibleCatalog.length"
              class="text-muted-foreground py-2 text-sm"
              data-testid="install-catalog-empty"
            >
              {{ query.trim()
                ? "No skills match your search."
                : store.loading ? "Loading catalog…" : "No catalog skills available." }}
            </p>
          </div>
        </TabsContent>

        <TabsContent value="import" class="mt-4 grid min-w-0 gap-4">
          <div class="rounded-xl border p-5">
            <Label for="install-import-file" class="text-sm font-medium">Import a SKILL.md</Label>
            <p class="text-muted-foreground mt-1 text-sm">
              A Markdown file with name and description front matter.
            </p>
            <input
              id="install-import-file"
              type="file"
              accept=".md,text/markdown"
              data-testid="install-import-file"
              class="text-muted-foreground mt-4 block w-full text-sm file:mr-3 file:rounded-md file:border-0 file:bg-secondary file:px-3 file:py-1.5 file:text-sm file:font-medium file:text-secondary-foreground"
              @change="onFile"
            />
            <p v-if="fileName" class="text-muted-foreground mt-2 truncate text-xs" data-testid="install-import-file-name">
              {{ fileName }}
            </p>
            <Button
              type="button"
              size="sm"
              class="mt-4"
              :disabled="!skillMd || store.saving"
              data-testid="install-import-install"
              @click="install({ import: { skillMd } })"
            >
              Install
            </Button>
          </div>

        </TabsContent>
      </Tabs>

      <div class="mt-auto flex flex-wrap items-center gap-2 border-t pt-4">
        <span class="text-muted-foreground text-sm">Install into</span>
        <div class="flex shrink-0 items-center gap-2">
          <span
            class="text-sm"
            :class="scope === 'project' ? 'text-foreground font-medium' : 'text-muted-foreground'"
          >
            This project
          </span>
          <Switch
            :model-value="scope === 'global'"
            aria-label="Install globally"
            data-testid="install-scope-global"
            @update:model-value="(value) => (scope = value ? 'global' : 'project')"
          />
          <span
            class="text-sm"
            :class="scope === 'global' ? 'text-foreground font-medium' : 'text-muted-foreground'"
          >
            Global
          </span>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
