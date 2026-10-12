<script setup lang="ts">
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import type { ConnectionStatus } from "@/core/types";

defineProps<{ status: ConnectionStatus }>();

function statusLabel(status: ConnectionStatus) {
  return status === "open" ? "Connected" : status[0]!.toUpperCase() + status.slice(1);
}
</script>

<template>
  <TooltipProvider>
    <Tooltip>
      <TooltipTrigger as-child>
        <span
          role="status"
          tabindex="0"
          class="flex size-9 items-center justify-center rounded-lg outline-none focus-visible:ring-2 focus-visible:ring-ring"
          :data-status="status"
          :aria-label="statusLabel(status)"
        >
          <span class="size-2.5 rounded-full" :class="status === 'open' ? 'bg-emerald-500' : 'bg-amber-500'" />
        </span>
      </TooltipTrigger>
      <TooltipContent side="right">{{ statusLabel(status) }}</TooltipContent>
    </Tooltip>
  </TooltipProvider>
</template>
