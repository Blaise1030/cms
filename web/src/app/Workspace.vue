<script setup lang="ts">
import { ref } from "vue";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { Project } from "@/core/types";
import CheckpointsPanel from "@/modules/checkpoints/CheckpointsPanel.vue";
import FilesPanel from "@/modules/files/FilesPanel.vue";
import PreviewPanel from "@/modules/preview/PreviewPanel.vue";

defineProps<{ project: Project }>();

type Tab = "preview" | "files" | "history";
const tab = ref<Tab>("preview");
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <Tabs v-model="tab" class="shrink-0 border-b px-3 py-1.5">
      <TabsList>
        <TabsTrigger value="preview">Preview</TabsTrigger>
        <TabsTrigger value="files">Files</TabsTrigger>
        <TabsTrigger value="history">History</TabsTrigger>
      </TabsList>
    </Tabs>
    <div class="min-h-0 flex-1">
      <!-- The preview stays mounted so switching tabs does not reload the iframe. -->
      <PreviewPanel v-show="tab === 'preview'" :project-id="project.id" :devserver="project.devserver" />
      <FilesPanel v-if="tab === 'files'" :project-id="project.id" />
      <CheckpointsPanel v-if="tab === 'history'" :project-id="project.id" />
    </div>
  </div>
</template>
