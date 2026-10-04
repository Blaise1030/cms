<script setup lang="ts">
import { RefreshCw, RotateCcw } from "@lucide/vue";
import {
  Menubar,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarRadioGroup,
  MenubarRadioItem,
  MenubarSeparator,
  MenubarTrigger,
} from "@/components/ui/menubar";
import type { WorkspaceTab } from "@/app/Shell.vue";
import { usePreviewStore, type Device } from "@/modules/preview/store";

defineProps<{ projectId: string; tab: WorkspaceTab }>();

const emit = defineEmits<{
  "update:tab": [tab: WorkspaceTab];
}>();

const preview = usePreviewStore();

const tabs: { id: WorkspaceTab; label: string }[] = [
  { id: "preview", label: "Preview" },
  { id: "files", label: "Code" },
  { id: "history", label: "History" },
];

const devices: { id: Device; label: string }[] = [
  { id: "desktop", label: "Desktop" },
  { id: "tablet", label: "Tablet" },
  { id: "mobile", label: "Mobile" },
];
</script>

<template>
  <Menubar class="h-8 border-none bg-transparent p-0 shadow-none" data-testid="editor-menubar">
    <MenubarMenu>
      <MenubarTrigger>View</MenubarTrigger>
      <MenubarContent>
        <MenubarRadioGroup
          :model-value="tab"
          @update:model-value="emit('update:tab', $event as WorkspaceTab)"
        >
          <MenubarRadioItem v-for="t in tabs" :key="t.id" :value="t.id">
            {{ t.label }}
          </MenubarRadioItem>
        </MenubarRadioGroup>
      </MenubarContent>
    </MenubarMenu>

    <MenubarMenu>
      <MenubarTrigger>Preview</MenubarTrigger>
      <MenubarContent>
        <MenubarRadioGroup
          :model-value="preview.device"
          @update:model-value="preview.setDevice($event as Device)"
        >
          <MenubarRadioItem v-for="d in devices" :key="d.id" :value="d.id">
            {{ d.label }}
          </MenubarRadioItem>
        </MenubarRadioGroup>
        <MenubarSeparator />
        <MenubarItem @select="preview.reload()">
          <RefreshCw />
          Reload preview
        </MenubarItem>
        <MenubarItem :disabled="preview.restarting" @select="preview.restart(projectId)">
          <RotateCcw />
          Restart dev server
        </MenubarItem>
      </MenubarContent>
    </MenubarMenu>
  </Menubar>
</template>
