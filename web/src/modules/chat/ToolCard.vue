<script setup lang="ts">
import { CircleAlert, CircleCheck, LoaderCircle } from "@lucide/vue";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import type { ToolView } from "./types";

defineProps<{ tool: ToolView }>();
</script>

<template>
  <Card class="flex-row items-center gap-2 px-3 py-2 text-xs" data-testid="tool-card">
    <LoaderCircle v-if="tool.state === 'running'" class="text-muted-foreground size-3.5 animate-spin" />
    <CircleCheck v-else-if="tool.state === 'done'" class="size-3.5 text-emerald-500" />
    <CircleAlert v-else class="text-destructive size-3.5" />
    <span class="min-w-0 flex-1 truncate font-mono" :title="tool.summary">
      {{ tool.summary || tool.tool }}
    </span>
    <Badge variant="outline" data-testid="tool-name">{{ tool.tool }}</Badge>
    <Badge
      :variant="tool.state === 'failed' ? 'destructive' : tool.state === 'done' ? 'secondary' : 'outline'"
      data-testid="tool-state"
    >
      {{ tool.state }}
    </Badge>
  </Card>
</template>
