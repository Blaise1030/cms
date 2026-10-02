<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import DaemonUnreachable from "@/app/DaemonUnreachable.vue";
import EditorMenubar from "@/app/EditorMenubar.vue";
import Shell, { type WorkspaceTab } from "@/app/Shell.vue";
import Workspace from "@/app/Workspace.vue";
import { useDaemon } from "@/core/daemon";
import ChatPanel from "@/modules/chat/ChatPanel.vue";
import { useChatStore } from "@/modules/chat/store";
import GithubAccount from "@/modules/github/GithubAccount.vue";
import ProjectsGrid from "@/modules/projects/ProjectsGrid.vue";
import { useProjectsStore } from "@/modules/projects/store";
import { toast } from "vue-sonner";

const daemon = useDaemon();
const projects = useProjectsStore();
const chat = useChatStore();
const tab = ref<WorkspaceTab>("preview");

// Keep the offline screen up (with a spinner) while a manual retry is in flight, instead of
// flashing the shell back in.
const retrying = ref(false);
const showOffline = computed(() => daemon.status.value === "unreachable" || retrying.value);

watch(daemon.status, (s) => {
  if (s === "open" || s === "unreachable") retrying.value = false;
  // First successful connect (or a retry after the daemon was down): fetch the project list.
  if (s === "open" && !projects.loaded) void projects.load();
});

watch(
  () => projects.selected?.id,
  () => {
    tab.value = "preview";
  },
);

async function toggleAutoApprove() {
  const p = projects.selected;
  if (!p) return;
  try {
    await projects.setAutoApprove(p.id, !p.autoApprove);
  } catch (e) {
    toast.error(`Could not update auto-approve: ${e instanceof Error ? e.message : String(e)}`);
  }
}

function retry() {
  retrying.value = true;
  daemon.retry();
}

function connect(link: string) {
  retrying.value = daemon.connectTo(link);
}

onMounted(() => daemon.start());
onBeforeUnmount(() => daemon.stop());
</script>

<template>
  <DaemonUnreachable
    v-if="showOffline"
    class="dark bg-background text-foreground"
    :origin="daemon.config.value.origin"
    :reason="daemon.reason.value"
    :connecting="retrying"
    @retry="retry"
    @connect="connect"
  />
  <Shell
    v-else
    :status="daemon.status.value"
    :project-name="projects.selected?.name"
  >
    <template #menubar>
      <EditorMenubar
        v-if="projects.selected"
        class="ml-2"
        :project-id="projects.selected.id"
        :tab="tab"
        @update:tab="tab = $event"
        @back="projects.select(null)"
      />
    </template>
    <template #account>
      <GithubAccount />
    </template>
    <template #workspace>
      <Workspace v-if="projects.selected" :project="projects.selected" :tab="tab" />
      <ProjectsGrid v-else @deleted="chat.forget" />
    </template>
    <template #chat>
      <ChatPanel
        v-if="projects.selected"
        :key="projects.selected.id"
        :project-id="projects.selected.id"
        :auto-approve="projects.selected.autoApprove"
        @toggle-auto-approve="toggleAutoApprove"
      />
      <div
        v-else
        class="text-muted-foreground flex h-full items-center justify-center p-6 text-center text-sm"
      >
        Open a project to start chatting with the agent.
      </div>
    </template>
  </Shell>
</template>
