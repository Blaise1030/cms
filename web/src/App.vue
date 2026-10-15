<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { RouterView } from "vue-router";
import CommandPalette from "@/app/CommandPalette.vue";
import DaemonUnreachable from "@/app/DaemonUnreachable.vue";
import SetupCheck from "@/app/SetupCheck.vue";
import UpdateNotice from "@/app/UpdateNotice.vue";
import { SETUP_WAIT_MS, dismissSetup, setupDismissed, setupIncomplete } from "@/app/setup";
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

// First-run setup check: shown instead of the shell when something is missing and the user
// hasn't moved on. Until the first status arrives the shell waits (at most SETUP_WAIT_MS from
// load); a failed check, or an older daemon without the endpoint, opens the shell.
const setup = ref<SetupStatus | null>(null);
const setupSkipped = ref(setupDismissed());
const setupPending = ref(!setupSkipped.value);
const showSetup = computed(() => !setupSkipped.value && !!setup.value && setupIncomplete(setup.value));

const setupTimer = setupPending.value ? setTimeout(() => (setupPending.value = false), SETUP_WAIT_MS) : undefined;
let setupRequested = false;
watch(
  daemon.status,
  (s) => {
    if (s !== "open" || setupRequested || !setupPending.value) return;
    setupRequested = true;
    daemon.api
      .setupStatus()
      .then((v) => {
        if (setupPending.value) setup.value = v;
      }, () => {})
      .finally(() => {
        clearTimeout(setupTimer);
        setupPending.value = false;
      });
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
onBeforeUnmount(() => {
  clearTimeout(setupTimer);
  daemon.stop();
});
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
  <div v-else-if="setupPending" class="bg-background h-svh" aria-busy="true" data-testid="setup-pending" />
  <SetupCheck
    v-else-if="showSetup"
    class="bg-background text-foreground"
    :initial="setup ?? undefined"
    @continue="continueFromSetup"
  />
  <div v-else class="bg-background text-foreground h-svh overflow-hidden">
    <RouterView />
    <CommandPalette />
    <UpdateNotice />
    <Toaster :theme="theme.resolved.value" />
  </div>
</template>
