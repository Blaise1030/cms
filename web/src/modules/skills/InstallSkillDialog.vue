<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Sparkles } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
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

const error = computed(() => store.error);

watch(open, (isOpen) => {
  if (!isOpen) return;
  segment.value = "catalog";
  scope.value = "project";
  skillMd.value = "";
  fileName.value = "";
  blankName.value = "";
  blankDescription.value = "";
});

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
    <DialogContent class="sm:max-w-lg" data-testid="install-skill-dialog">
      <DialogHeader>
        <DialogTitle>Install skill</DialogTitle>
        <DialogDescription>
          Add a built-in skill, or import your own SKILL.md.
        </DialogDescription>
      </DialogHeader>

      <div class="grid gap-4">
        <div class="flex items-center gap-2">
          <span class="text-muted-foreground text-sm">Install into</span>
          <button
            type="button"
            :aria-pressed="scope === 'project'"
            data-testid="install-scope-project"
            class="rounded-md px-2 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            :class="scope === 'project' ? 'bg-accent text-accent-foreground font-medium' : 'text-muted-foreground hover:text-foreground'"
            @click="scope = 'project'"
          >
            This project
          </button>
          <button
            type="button"
            :aria-pressed="scope === 'global'"
            data-testid="install-scope-global"
            class="rounded-md px-2 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            :class="scope === 'global' ? 'bg-accent text-accent-foreground font-medium' : 'text-muted-foreground hover:text-foreground'"
            @click="scope = 'global'"
          >
            Global
          </button>
        </div>

        <p v-if="error" role="alert" class="text-destructive text-sm" data-testid="install-error">
          {{ error }}
        </p>

        <Tabs v-model="segment">
          <TabsList>
            <TabsTrigger value="catalog" data-testid="install-segment-catalog">Catalog</TabsTrigger>
            <TabsTrigger value="import" data-testid="install-segment-import">Import</TabsTrigger>
          </TabsList>

          <TabsContent value="catalog" class="grid gap-2">
            <div
              v-for="item in store.catalog"
              :key="item.id"
              data-testid="install-catalog-item"
              :data-skill-id="item.id"
              class="flex items-center justify-between gap-3 rounded-lg border p-3"
            >
              <div class="min-w-0">
                <p class="text-sm font-medium">{{ item.name }}</p>
                <p class="text-muted-foreground truncate text-xs">{{ item.description }}</p>
              </div>
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
            </div>
          </TabsContent>

          <TabsContent value="import" class="grid gap-4">
            <div class="space-y-1.5">
              <Label for="install-import-file" class="text-xs font-medium">Import a SKILL.md</Label>
              <input
                id="install-import-file"
                type="file"
                accept=".md,text/markdown"
                data-testid="install-import-file"
                class="text-sm"
                @change="onFile"
              />
              <p v-if="fileName" class="text-muted-foreground text-xs" data-testid="install-import-file-name">
                {{ fileName }}
              </p>
              <Button
                type="button"
                size="sm"
                :disabled="!skillMd || store.saving"
                data-testid="install-import-install"
                @click="install({ import: { skillMd } })"
              >
                Install
              </Button>
            </div>

            <div class="rounded-lg border p-3">
              <p class="mb-2 flex items-center gap-1.5 text-sm font-medium">
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
