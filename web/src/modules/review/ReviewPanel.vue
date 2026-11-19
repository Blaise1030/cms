<script setup lang="ts">
import ReviewChange from "./ReviewChange.vue";
import ReviewInbox from "./ReviewInbox.vue";

import type { ReviewPage } from "@/router";

defineProps<{ projectId: string; number?: number; page?: ReviewPage }>();
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="review-panel">
    <ReviewChange v-if="number" :project-id="projectId" :number="number" :page="page ?? 'conversation'" />
    <ReviewInbox v-else :project-id="projectId" />
  </div>
</template>
