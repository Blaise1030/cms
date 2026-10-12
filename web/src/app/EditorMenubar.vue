<script setup lang="ts">
import { ButtonGroup, ButtonGroupSeparator } from "@/components/ui/button-group";
import ChangesBranchPicker from "@/modules/changes/ChangesBranchPicker.vue";
import GetLatestButton from "@/modules/changes/GetLatestButton.vue";

defineProps<{ projectId: string }>();
</script>

<template>
  <ButtonGroup>
    <ChangesBranchPicker :project-id="projectId" />
    <ButtonGroupSeparator />
    <GetLatestButton :project-id="projectId" />
  </ButtonGroup>
</template>
