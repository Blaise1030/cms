<script setup lang="ts">
import { TooltipProvider } from "reka-ui";
import UChip from "@nuxt/ui/components/Chip.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import type { ConnectionStatus } from "@/core/types";

defineProps<{ status: ConnectionStatus }>();

function statusLabel(status: ConnectionStatus) {
  return status === "open" ? "Connected" : status[0]!.toUpperCase() + status.slice(1);
}
</script>

<template>
  <!-- Own provider: UApp's covers the app, this one keeps the status dot working when mounted alone. -->
  <TooltipProvider>
    <UTooltip :text="statusLabel(status)" :content="{ side: 'right' }">
      <span
        role="status"
        tabindex="0"
        class="focus-visible:ring-primary flex size-9 items-center justify-center rounded-lg outline-none focus-visible:ring-2"
        :data-status="status"
        :aria-label="statusLabel(status)"
      >
        <UChip standalone inset size="xl" :color="status === 'open' ? 'success' : 'warning'" />
      </span>
    </UTooltip>
  </TooltipProvider>
</template>
