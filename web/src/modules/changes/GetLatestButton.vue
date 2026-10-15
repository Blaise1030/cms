<script setup lang="ts">
import { RefreshCw } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { getLatest } from "./getLatest";
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
    aria-label="Sync with live site"
    title="Sync with live site"
    :disabled="store.updating || store.switching || store.creating"
    @click="getLatest(projectId)"
  >
    <RefreshCw class="size-4" :class="store.updating ? 'animate-spin' : ''" />
  </Button>
</template>
