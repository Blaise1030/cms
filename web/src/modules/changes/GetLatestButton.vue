<script setup lang="ts">
import { RefreshCw } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { askAgentToGetLatest } from "./agentRequests";
import { useChangesStore } from "./store";

defineProps<{ projectId: string }>();
const store = useChangesStore();
</script>

<template>
  <Button
    variant="secondary"
    size="icon"
    class="size-7"
    data-testid="get-latest"
    aria-label="Sync from main"
    title="Sync from main: ask the agent to bring the latest version of the site into this change"
    :disabled="store.updating || store.switching || store.creating"
    @click="askAgentToGetLatest()"
  >
    <RefreshCw class="size-4" :class="store.updating ? 'animate-spin' : ''" />
  </Button>
</template>
