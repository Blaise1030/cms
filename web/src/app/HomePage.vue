<script setup lang="ts">
import { useChatStore } from "@/modules/chat/store";
import ProjectsGrid from "@/modules/projects/ProjectsGrid.vue";

const chat = useChatStore();
</script>

<template>
  <ProjectsGrid @deleted="chat.forget" />
</template>
