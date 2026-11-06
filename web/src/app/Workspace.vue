<script setup lang="ts">
import { computed } from "vue";
import { RouterView, useRoute } from "vue-router";
import type { Project } from "@/core/types";
import { useChatPanelLayout } from "@/lib/chatPanelLayout";
import { projectTabFromRoute } from "@/router";
import PreviewPanel from "@/modules/preview/PreviewPanel.vue";
import ConflictView from "@/modules/changes/ConflictView.vue";
import { useChangesStore } from "@/modules/changes/store";

defineProps<{ project: Project }>();

const route = useRoute();
const tab = computed(() => projectTabFromRoute(route.name));
const changes = useChangesStore();
const panelLayout = useChatPanelLayout();
/** Leave room for the floating agent session bar under the workspace (not the rail). */
const floatingChrome = computed(() => panelLayout.floating.value);
</script>

<template>
  <div
    class="flex h-full min-h-0 flex-col"
    :class="floatingChrome ? 'pb-11' : undefined"
    data-testid="workspace-root"
  >
    <div class="relative min-h-0 flex-1">
      <!-- The preview stays mounted and laid out so switching tabs does not reload or re-measure it. -->
      <PreviewPanel
        class="absolute inset-0"
        :class="tab === 'preview' ? undefined : 'invisible'"
        :inert="tab !== 'preview'"
        :project-id="project.id"
        :devserver="project.devserver"
        :port="project.port"
      />
      <div v-show="tab !== 'preview'" class="bg-background absolute inset-0" data-testid="workspace-surface">
        <RouterView />
      </div>
      <ConflictView v-if="changes.selectedConflictId" :project-id="project.id" />
    </div>
  </div>
</template>
