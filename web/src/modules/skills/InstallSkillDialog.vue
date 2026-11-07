<script setup lang="ts">
import { computed, ref, watch, type Component } from "vue";
import { Palette, Search, Sparkles } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item";
import { Label } from "@/components/ui/label";
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
const skillMd = ref("");
const fileName = ref("");
const blankName = ref("");
const blankDescription = ref("");

const error = computed(() => store.error ?? store.conflict?.message ?? null);
const CATALOG_ICON: Record<string, Component> = { seo: Search, design: Palette };
const catalogIcon = (id: string) => CATALOG_ICON[id] ?? Sparkles;

watch(open, (isOpen) => {
  if (!isOpen) return;
  // A fresh open starts clean: a stale install error/conflict must not linger.
  store.error = null;
  store.conflict = null;
  segment.value = "catalog";
  scope.value = "project";
  skillMd.value = "";
  fileName.value = "";
  blankName.value = "";
  blankDescription.value = "";
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
    <DialogContent class="sm:max-w-xl" data-testid="install-skill-dialog">
      <DialogHeader>
        <DialogTitle>Install skill</DialogTitle>
        <DialogDescription>
          Add a built-in skill, or import your own SKILL.md.
        </DialogDescription>
      </DialogHeader>

      <div class="grid min-w-0 gap-4">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <span class="text-muted-foreground text-sm">Install into</span>
          <div class="bg-muted/40 inline-flex shrink-0 rounded-lg border p-0.5" role="group" aria-label="Install scope">
            <button
              type="button"
              :aria-pressed="scope === 'project'"
              data-testid="install-scope-project"
              class="rounded-md px-3 py-1.5 text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring"
              :class="scope === 'project' ? 'bg-background text-foreground font-medium shadow-sm' : 'text-muted-foreground hover:text-foreground'"
              @click="scope = 'project'"
            >
              This project
            </button>
            <button
              type="button"
              :aria-pressed="scope === 'global'"
              data-testid="install-scope-global"
              class="rounded-md px-3 py-1.5 text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring"
              :class="scope === 'global' ? 'bg-background text-foreground font-medium shadow-sm' : 'text-muted-foreground hover:text-foreground'"
              @click="scope = 'global'"
            >
              Global
            </button>
          </div>
        </div>

        <p v-if="error" role="alert" class="text-destructive text-sm" data-testid="install-error">
          {{ error }}
        </p>

        <Tabs v-model="segment" class="min-w-0">
          <TabsList>
            <TabsTrigger value="catalog" data-testid="install-segment-catalog">Catalog</TabsTrigger>
            <TabsTrigger value="import" data-testid="install-segment-import">Import</TabsTrigger>
          </TabsList>

          <TabsContent value="catalog" class="min-w-0">
            <div class="max-h-[45vh] overflow-y-auto pr-1">
              <ItemGroup class="gap-2">
                <Item
                  v-for="item in store.catalog"
                  :key="item.id"
                  variant="outline"
                  size="sm"
                  data-testid="install-catalog-item"
                  :data-skill-id="item.id"
                  class="min-w-0"
                >
                  <ItemMedia variant="icon">
                    <component :is="catalogIcon(item.id)" aria-hidden="true" />
                  </ItemMedia>
                  <ItemContent class="min-w-0">
                    <ItemTitle>{{ item.name }}</ItemTitle>
                    <ItemDescription>{{ item.description }}</ItemDescription>
                  </ItemContent>
                  <ItemActions class="shrink-0">
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      :disabled="installedInScope(item) || store.saving"
                      data-testid="install-catalog-install"
                      :data-skill-id="item.id"
                      @click="install({ catalog: { id: item.id } })"
                    >
                      {{ installedInScope(item) ? "Installed" : "Install" }}
                    </Button>
                  </ItemActions>
                </Item>
              </ItemGroup>
              <p
                v-if="!store.catalog.length"
                class="text-muted-foreground py-2 text-xs"
                data-testid="install-catalog-empty"
              >
                {{ store.loading ? "Loading catalog…" : "No catalog skills available." }}
              </p>
            </div>
          </TabsContent>

          <TabsContent value="import" class="grid min-w-0 gap-4">
            <div class="rounded-lg border p-4">
              <Label for="install-import-file" class="text-sm font-medium">Import a SKILL.md</Label>
              <p class="text-muted-foreground mt-1 text-xs">
                A Markdown file with name and description front matter.
              </p>
              <input
                id="install-import-file"
                type="file"
                accept=".md,text/markdown"
                data-testid="install-import-file"
                class="text-muted-foreground mt-3 block w-full text-sm file:mr-3 file:rounded-md file:border-0 file:bg-secondary file:px-3 file:py-1.5 file:text-sm file:font-medium file:text-secondary-foreground"
                @change="onFile"
              />
              <p v-if="fileName" class="text-muted-foreground mt-2 truncate text-xs" data-testid="install-import-file-name">
                {{ fileName }}
              </p>
              <Button
                type="button"
                size="sm"
                class="mt-3"
                :disabled="!skillMd || store.saving"
                data-testid="install-import-install"
                @click="install({ import: { skillMd } })"
              >
                Install
              </Button>
            </div>

            <div class="rounded-lg border p-4">
              <p class="mb-3 flex items-center gap-1.5 text-sm font-medium">
                <Sparkles class="size-3.5" aria-hidden="true" /> Create a blank skill
              </p>
              <div class="grid gap-2">
                <Input
                  v-model="blankName"
                  placeholder="Name"
                  data-testid="install-blank-name"
                  class="h-8"
                />
                <Input
                  v-model="blankDescription"
                  placeholder="Description"
                  data-testid="install-blank-description"
                  class="h-8"
                />
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  class="self-start"
                  :disabled="!blankName.trim() || !blankDescription.trim() || store.saving"
                  data-testid="install-blank-create"
                  @click="install({ blank: { name: blankName.trim(), description: blankDescription.trim() } })"
                >
                  Create blank
                </Button>
              </div>
            </div>
          </TabsContent>
        </Tabs>
      </div>
    </DialogContent>
  </Dialog>
</template>
