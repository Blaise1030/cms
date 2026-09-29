<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import DaemonUnreachable from "@/app/DaemonUnreachable.vue";
import Shell from "@/app/Shell.vue";
import { useDaemon } from "@/core/daemon";

const daemon = useDaemon();

// Keep the offline screen up (with a spinner) while a manual retry is in flight, instead of
// flashing the shell back in.
const retrying = ref(false);
const showOffline = computed(() => daemon.status.value === "unreachable" || retrying.value);

watch(daemon.status, (s) => {
  if (s === "open" || s === "unreachable") retrying.value = false;
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
  <Shell v-else :status="daemon.status.value" />
</template>
