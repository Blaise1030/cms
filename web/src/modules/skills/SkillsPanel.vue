<script setup lang="ts">
import { watch } from "vue";
import { useSkillsStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useSkillsStore();

watch(() => props.projectId, id => void store.open(id), { immediate: true });
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="skills-panel">
    <p class="p-4 text-sm text-muted-foreground">Skills</p>
  </div>
</template>
