<script setup lang="ts">
import type { ToolUIPart } from "ai";
import { Tool, ToolHeader } from "@/components/ai-elements/tool";
import type { ToolView } from "./types";

defineProps<{ tool: ToolView }>();

const STATE: Record<ToolView["state"], ToolUIPart["state"]> = {
  running: "input-available",
  done: "output-available",
  failed: "output-error",
};
</script>

<template>
  <Tool class="mb-0" data-testid="tool-card">
    <ToolHeader
      type="dynamic-tool"
      :tool-name="tool.tool"
      :state="STATE[tool.state]"
      :title="tool.summary || tool.tool"
    />
    <span class="sr-only" data-testid="tool-name">{{ tool.tool }}</span>
    <span class="sr-only" data-testid="tool-state">{{ tool.state }}</span>
  </Tool>
</template>
