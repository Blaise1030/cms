<script setup lang="ts">
import { computed } from "vue";
import { RefreshCw } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { getLatest } from "./getLatest";
import { useChangesStore } from "./store";

defineProps<{ projectId: string }>();
const store = useChangesStore();

/** Null while the daemon hasn't said — keep it enabled rather than wrongly disabling. */
const nothingToSync = computed(() => store.syncNeeded === false);
const title = computed(() => (nothingToSync.value ? "Already up to date" : "Sync with live site"));
</script>

<template>
  <Button
    variant="outline"
    size="icon"
    class="bg-card size-7"
    data-testid="get-latest"
    aria-label="Sync with live site"
    :title="title"
    :disabled="store.updating || store.switching || store.creating || nothingToSync"
    @click="getLatest(projectId)"
  >
    <RefreshCw class="size-4" :class="store.updating ? 'animate-spin' : ''" />
  </Button>
</template>
