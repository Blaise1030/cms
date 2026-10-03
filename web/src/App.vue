<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { RouterView } from "vue-router";
import DaemonUnreachable from "@/app/DaemonUnreachable.vue";
import { Toaster } from "@/components/ui/sonner";
import { useDaemon } from "@/core/daemon";
import { useProjectsStore } from "@/modules/projects/store";

const daemon = useDaemon();
const projects = useProjectsStore();

// Keep the offline screen up (with a spinner) while a manual retry is in flight, instead of
// flashing the app back in.
const retrying = ref(false);
const showOffline = computed(() => daemon.status.value === "unreachable" || retrying.value);

watch(daemon.status, (s) => {
  if (s === "open" || s === "unreachable") retrying.value = false;
  // First successful connect (or a retry after the daemon was down): fetch the project list.
  if (s === "open" && !projects.loaded) void projects.load();
});

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
  <div v-else class="dark bg-background text-foreground h-svh overflow-hidden">
    <RouterView />
    <Toaster />
  </div>
</template>
