<script setup lang="ts">
import { computed } from "vue";
import { RouterView, useRoute } from "vue-router";
import type { Project } from "@/core/types";
import { projectTabFromRoute } from "@/router";
import PreviewPanel from "@/modules/preview/PreviewPanel.vue";
import ConflictView from "@/modules/changes/ConflictView.vue";
import SyncDialog from "@/modules/changes/SyncDialog.vue";
import { useChangesStore } from "@/modules/changes/store";

defineProps<{ project: Project }>();

const route = useRoute();
const tab = computed(() => projectTabFromRoute(route.name));
const changes = useChangesStore();
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="relative min-h-0 flex-1">
      <!-- The preview stays mounted so switching tabs does not reload the iframe. -->
      <PreviewPanel
        v-show="tab === 'preview'"
        class="absolute inset-0"
        :project-id="project.id"
        :devserver="project.devserver"
      />
      <div v-show="tab !== 'preview'" class="absolute inset-0">
        <RouterView />
      </div>
      <ConflictView v-if="changes.selectedConflictId" :project-id="project.id" />
      <SyncDialog v-if="changes.syncSessionId" :project-id="project.id" />
    </div>
  </div>
</template>
