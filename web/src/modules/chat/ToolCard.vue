<script setup lang="ts">
import { computed } from "vue";
import {
  ArrowRightLeft,
  Brain,
  CircleAlert,
  Eye,
  FilePen,
  Globe,
  Loader2,
  Search,
  SquareTerminal,
  Trash2,
  Wrench,
} from "@lucide/vue";
import type { ToolView } from "./types";

const props = defineProps<{ tool: ToolView }>();

// Keyed by the ACP tool-call `kind` the daemon forwards as `tool`.
const ICON: Record<string, typeof Wrench> = {
  read: Eye,
  edit: FilePen,
  delete: Trash2,
  move: ArrowRightLeft,
  search: Search,
  execute: SquareTerminal,
  think: Brain,
  fetch: Globe,
};

const icon = computed(() => {
  if (props.tool.state === "running") return Loader2;
  if (props.tool.state === "failed") return CircleAlert;
  return ICON[props.tool.tool] ?? Wrench;
});
</script>

<template>
  <div
    class="text-muted-foreground flex min-w-0 items-center gap-2 py-0.5 text-sm"
    :class="tool.state === 'failed' && 'text-destructive'"
    :title="tool.summary || tool.tool"
    data-testid="tool-card"
  >
    <component
      :is="icon"
      class="size-3.5 shrink-0"
      :class="tool.state === 'running' && 'animate-spin'"
    />
    <span class="truncate">{{ tool.summary || tool.tool }}</span>
    <span class="sr-only" data-testid="tool-name">{{ tool.tool }}</span>
    <span class="sr-only" data-testid="tool-state">{{ tool.state }}</span>
  </div>
</template>
