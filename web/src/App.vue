<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { RouterView } from "vue-router";
import CommandPalette from "@/app/CommandPalette.vue";
import DaemonUnreachable from "@/app/DaemonUnreachable.vue";
import SetupCheck from "@/app/SetupCheck.vue";
import { dismissSetup, setupDismissed, setupIncomplete } from "@/app/setup";
import { Toaster } from "@/components/ui/sonner";
import { useDaemon } from "@/core/daemon";
import type { SetupStatus } from "@/core/types";
import { useTheme } from "@/lib/theme";
import { useProjectsStore } from "@/modules/projects/store";

const daemon = useDaemon();
const projects = useProjectsStore();
const theme = useTheme();

// Keep the offline screen up (with a spinner) while a manual retry is in flight, instead of
// flashing the app back in.
const retrying = ref(false);
const showOffline = computed(() => daemon.status.value === "unreachable" || retrying.value);

watch(daemon.status, (s) => {
  if (s === "open" || s === "unreachable") retrying.value = false;
  // First successful connect (or a retry after the daemon was down): fetch the project list.
  if (s === "open" && !projects.loaded) void projects.load();
});

// First-run setup check: only shown when something is missing and the user hasn't moved on.
// It never holds the shell back; an older daemon without the endpoint just skips it.
const setup = ref<SetupStatus | null>(null);
const setupSkipped = ref(setupDismissed());
const showSetup = computed(() => !setupSkipped.value && !!setup.value && setupIncomplete(setup.value));

watch(
  daemon.status,
  (s) => {
    if (s !== "open" || setup.value || setupSkipped.value) return;
    daemon.api.setupStatus().then((v) => (setup.value = v), () => {});
  },
  { immediate: true },
);

function continueFromSetup() {
  dismissSetup();
  setupSkipped.value = true;
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
    class="bg-background text-foreground"
    :origin="daemon.config.value.origin"
    :reason="daemon.reason.value"
    :connecting="retrying"
    @retry="retry"
    @connect="connect"
  />
  <SetupCheck
    v-else-if="showSetup"
    class="bg-background text-foreground"
    :initial="setup ?? undefined"
    @continue="continueFromSetup"
  />
  <div v-else class="bg-background text-foreground h-svh overflow-hidden">
    <RouterView />
    <CommandPalette />
    <Toaster :theme="theme.resolved.value" />
  </div>
</template>
