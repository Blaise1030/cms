<script setup lang="ts">
import type { Project } from "@/core/types";
import type { WorkspaceTab } from "@/app/Shell.vue";
import CheckpointsPanel from "@/modules/checkpoints/CheckpointsPanel.vue";
import FilesPanel from "@/modules/files/FilesPanel.vue";
import PreviewPanel from "@/modules/preview/PreviewPanel.vue";

defineProps<{ project: Project; tab: WorkspaceTab }>();
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="min-h-0 flex-1">
      <!-- The preview stays mounted so switching tabs does not reload the iframe. -->
      <PreviewPanel
        v-show="tab === 'preview'"
        :project-id="project.id"
        :devserver="project.devserver"
      />
      <FilesPanel v-if="tab === 'files'" :project-id="project.id" />
      <CheckpointsPanel v-if="tab === 'history'" :project-id="project.id" />
    </div>
  </div>
</template>
