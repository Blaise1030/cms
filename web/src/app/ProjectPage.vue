<script setup lang="ts">
import { computed, onBeforeUnmount, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { toast } from "vue-sonner";
import EditorMenubar from "@/app/EditorMenubar.vue";
import Shell from "@/app/Shell.vue";
import Workspace from "@/app/Workspace.vue";
import { useDaemon } from "@/core/daemon";
import { errorMessage } from "@/lib/errors";
import SharePreviewButton from "@/modules/changes/SharePreviewButton.vue";
import { useChangesStore } from "@/modules/changes/store";
import GithubAccount from "@/modules/github/GithubAccount.vue";
import { useProjectsStore } from "@/modules/projects/store";
import TerminalPanel from "@/modules/terminal/TerminalPanel.vue";
import { projectTabFromRoute } from "@/router";

const props = defineProps<{ id: string }>();

const daemon = useDaemon();
const route = useRoute();
const router = useRouter();
const projects = useProjectsStore();
const changes = useChangesStore();

const project = computed(() => projects.projects.find((p) => p.id === props.id) ?? null);
const showChat = computed(() => projectTabFromRoute(route.name) === "preview");

watch(
  () => props.id,
  (id) => {
    void changes.load(id).then(() => changes.loadConflicts(id)).catch(() => undefined);
  },
  { immediate: true },
);
onBeforeUnmount(() => changes.stopStatusPolling());

// An unknown id (a stale link, or a project deleted elsewhere) goes back to the project list.
watch(
  () => projects.loaded && !project.value,
  (missing) => {
    if (!missing) return;
    toast.error(`Project "${props.id}" was not found`);
    void router.replace("/");
  },
  { immediate: true },
);

async function toggleAutoApprove() {
  const p = project.value;
  if (!p) return;
  try {
    await projects.setAutoApprove(p.id, !p.autoApprove);
  } catch (e) {
    toast.error(`Could not update auto-approve: ${errorMessage(e)}`);
  }
}
</script>

<template>
  <Shell :status="daemon.status.value" :show-chat="showChat">
    <template #menubar>
      <EditorMenubar v-if="project" :project-id="project.id" />
    </template>
    <template #actions>
      <SharePreviewButton v-if="project" :project-id="project.id" />
    </template>
    <template #account>
      <GithubAccount />
    </template>
    <template #workspace>
      <Workspace v-if="project" :project="project" />
    </template>
    <template #chat>
      <TerminalPanel
        v-if="project"
        :key="project.id"
        :project-id="project.id"
        :auto-approve="project.autoApprove"
        @toggle-auto-approve="toggleAutoApprove"
      />
    </template>
  </Shell>
</template>
