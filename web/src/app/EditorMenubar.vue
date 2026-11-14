<script setup lang="ts">
import ChangesBranchPicker from "@/modules/changes/ChangesBranchPicker.vue";
import GetLatestButton from "@/modules/changes/GetLatestButton.vue";

defineProps<{ projectId: string }>();
</script>

<template>
  <div class="flex items-center gap-1.5">
    <ChangesBranchPicker :project-id="projectId" />
    <GetLatestButton :project-id="projectId" />
  </div>
</template>
