<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import ChangesBranchPicker from "@/modules/changes/ChangesBranchPicker.vue";
import { projectPath, projectTabFromRoute, type ProjectTab } from "@/router";

const props = defineProps<{ projectId: string }>();

const route = useRoute();
const router = useRouter();

const tabs: { id: ProjectTab; label: string }[] = [
  { id: "preview", label: "Preview" },
  { id: "code", label: "Code" },
  { id: "history", label: "History" },
];

const tab = computed(() => projectTabFromRoute(route.name));

function onTab(value: string | number) {
  void router.push(projectPath(props.projectId, value as ProjectTab));
}
</script>

<template>
  <div class="flex min-w-0 items-center gap-3">
    <Tabs :model-value="tab" data-testid="workspace-tabs" @update:model-value="onTab">
      <TabsList class="h-7">
        <TabsTrigger
          v-for="t in tabs"
          :key="t.id"
          :value="t.id"
          class="h-6 px-2.5 text-xs"
          :data-testid="`workspace-tab-${t.id}`"
        >
          {{ t.label }}
        </TabsTrigger>
      </TabsList>
    </Tabs>

    <div class="bg-border h-4 w-px shrink-0" aria-hidden="true" />
    <ChangesBranchPicker :project-id="projectId" />
  </div>
</template>
