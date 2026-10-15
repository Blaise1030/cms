<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { toast } from "vue-sonner";
import ConnectionStatus from "@/app/ConnectionStatus.vue";
import EditorMenubar from "@/app/EditorMenubar.vue";
import ProjectRail from "@/app/ProjectRail.vue";
import Shell from "@/app/Shell.vue";
import Workspace from "@/app/Workspace.vue";
import { useDaemon } from "@/core/daemon";
import SharePreviewButton from "@/modules/changes/SharePreviewButton.vue";
import { report } from "@/modules/changes/getLatest";
import { useChangesStore } from "@/modules/changes/store";
import GithubAccount from "@/modules/github/GithubAccount.vue";
import { useProjectsStore } from "@/modules/projects/store";
import TerminalPanel from "@/modules/terminal/TerminalPanel.vue";
import { useAgentOutboxStore } from "@/modules/terminal/outbox";
import { useTerminalStore } from "@/modules/terminal/store";
import { projectTabFromRoute } from "@/router";

const props = defineProps<{ id: string }>();

const daemon = useDaemon();
const route = useRoute();
const router = useRouter();
const projects = useProjectsStore();
const changes = useChangesStore();
const terminal = useTerminalStore();

const outbox = useAgentOutboxStore();
const shell = ref<InstanceType<typeof Shell> | null>(null);

const project = computed(() => projects.projects.find((p) => p.id === props.id) ?? null);
/** Something was sent to the agent from a tab that normally hides it. */
const chatRequested = ref(false);
const showChat = computed(() => {
  const tab = projectTabFromRoute(route.name);
  return chatRequested.value || tab === "preview" || tab === "content" || tab === "design" || tab === "code";
});

watch(
  () => route.name,
  () => {
    chatRequested.value = false;
  },
);
watch(
  () => outbox.sent,
  async () => {
    chatRequested.value = true;
    await nextTick();
    shell.value?.openChat();
  },
);

watch(
  () => props.id,
  (id) => {
    void changes.load(id).then(() => changes.loadConflicts(id)).catch(() => undefined);
  },
  { immediate: true },
);
const offBlocked = daemon.on("checkpoint.blocked", (e) => {
  if (e.projectId !== props.id) return;
  toast.error(e.reason);
});
const offUpdated = daemon.on("changes.updated", (e) => {
  if (e.projectId !== props.id) return;
  if (e.error || !e.result) {
    toast.error(e.error ?? "Couldn't sync with main");
    return;
  }
  report(e.result);
  void changes.applyUpdate(props.id, e.result).catch(() => undefined);
});
const offAutoResolve = daemon.on("changes.auto_resolve", (e) => {
  if (e.projectId !== props.id) return;
  void changes.onAutoResolve(props.id, e).catch(() => undefined);
});
const offTurn = daemon.on("terminal.turn", (e) => {
  if (e.projectId !== props.id) return;
  void changes.followActive(props.id).catch(() => undefined);
});
onBeforeUnmount(() => {
  offBlocked();
  offUpdated();
  offAutoResolve();
  offTurn();
  changes.stopStatusPolling();
});

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
</script>

<template>
  <Shell ref="shell" :show-chat="showChat" :chat-detached="terminal.floating">
    <template #rail>
      <ProjectRail v-if="project" :project-id="project.id">
        <template #footer>
          <ConnectionStatus :status="daemon.status.value" />
          <GithubAccount rail />
        </template>
      </ProjectRail>
    </template>
    <template #menubar>
      <EditorMenubar v-if="project" :project-id="project.id" />
    </template>
    <template #actions>
      <SharePreviewButton v-if="project" :project-id="project.id" />
    </template>
    <template #workspace>
      <Workspace v-if="project" :project="project" />
    </template>
    <template #chat>
      <TerminalPanel
        v-if="project"
        :key="project.id"
        :project-id="project.id"
      />
    </template>
  </Shell>
</template>
