<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { toast } from "vue-sonner";
import EditorMenubar from "@/app/EditorMenubar.vue";
import Shell, { type WorkspaceTab } from "@/app/Shell.vue";
import Workspace from "@/app/Workspace.vue";
import { useDaemon } from "@/core/daemon";
import { errorMessage } from "@/lib/errors";
import ChatPanel from "@/modules/chat/ChatPanel.vue";
import { useChangesStore } from "@/modules/changes/store";
import GithubAccount from "@/modules/github/GithubAccount.vue";
import { useProjectsStore } from "@/modules/projects/store";

const props = defineProps<{ id: string }>();

const daemon = useDaemon();
const router = useRouter();
const projects = useProjectsStore();
const changes = useChangesStore();
const tab = ref<WorkspaceTab>("preview");

const project = computed(() => projects.projects.find((p) => p.id === props.id) ?? null);

watch(
  () => props.id,
  (id) => {
    tab.value = "preview";
    void changes.load(id);
  },
  { immediate: true },
);

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
  <Shell :status="daemon.status.value">
    <template #menubar>
      <EditorMenubar
        v-if="project"
        class="ml-2"
        :project-id="project.id"
        :tab="tab"
        @update:tab="tab = $event"
      />
    </template>
    <template #account>
      <GithubAccount />
    </template>
    <template #workspace>
      <Workspace v-if="project" :project="project" :tab="tab" />
    </template>
    <template #chat>
      <ChatPanel
        v-if="project"
        :key="project.id"
        :project-id="project.id"
        :auto-approve="project.autoApprove"
        @toggle-auto-approve="toggleAutoApprove"
      />
    </template>
  </Shell>
</template>
