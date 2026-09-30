<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import DaemonUnreachable from "@/app/DaemonUnreachable.vue";
import Shell from "@/app/Shell.vue";
import Workspace from "@/app/Workspace.vue";
import { Button } from "@/components/ui/button";
import { useDaemon } from "@/core/daemon";
import ChatPanel from "@/modules/chat/ChatPanel.vue";
import { useChatStore } from "@/modules/chat/store";
import ProjectsGrid from "@/modules/projects/ProjectsGrid.vue";
import { useProjectsStore } from "@/modules/projects/store";
import { toast } from "vue-sonner";
import { ArrowLeft } from "@lucide/vue";

const daemon = useDaemon();
const projects = useProjectsStore();
const chat = useChatStore();

// Keep the offline screen up (with a spinner) while a manual retry is in flight, instead of
// flashing the shell back in.
const retrying = ref(false);
const showOffline = computed(() => daemon.status.value === "unreachable" || retrying.value);

watch(daemon.status, (s) => {
  if (s === "open" || s === "unreachable") retrying.value = false;
  // First successful connect (or a retry after the daemon was down): fetch the project list.
  if (s === "open" && !projects.loaded) void projects.load();
});

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
  <Shell v-else :status="daemon.status.value">
    <template v-if="projects.selected" #header>
      <Button variant="ghost" size="sm" @click="projects.select(null)">
        <ArrowLeft /> Projects
      </Button>
      <span class="text-muted-foreground text-sm">{{ projects.selected.name }}</span>
    </template>
    <template #preview>
      <Workspace v-if="projects.selected" :project="projects.selected" />
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
      <div v-else class="text-muted-foreground flex h-full items-center justify-center p-6 text-center text-sm">
        Open a project to start chatting with the agent.
      </div>
    </template>
  </Shell>
</template>
