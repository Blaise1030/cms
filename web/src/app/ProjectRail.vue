<script setup lang="ts">
import { computed, type Component } from "vue";
import { RouterLink, useRoute } from "vue-router";
import { Code, FileText, History, House, MonitorPlay, Palette } from "@lucide/vue";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { projectPath, projectTabFromRoute, type ProjectTab } from "@/router";

const props = defineProps<{ projectId: string }>();

const route = useRoute();
const active = computed(() => projectTabFromRoute(route.name));

const tabs: { id: ProjectTab; label: string; icon: Component }[] = [
  { id: "preview", label: "Preview", icon: MonitorPlay },
  { id: "content", label: "Content", icon: FileText },
  { id: "design", label: "Design", icon: Palette },
  { id: "code", label: "Code", icon: Code },
  { id: "history", label: "History", icon: History },
];

const item = "relative flex size-9 items-center justify-center rounded-lg text-muted-foreground outline-none transition-colors hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring [&_svg]:size-5";
</script>

<template>
  <TooltipProvider :delay-duration="300">
    <nav class="flex h-full w-14 shrink-0 flex-col items-center gap-1 border-r bg-background py-2" aria-label="Project" data-testid="project-rail">
      <Tooltip>
        <TooltipTrigger as-child>
          <RouterLink to="/" :class="[item, 'mb-2 border bg-muted/50 text-foreground']" aria-label="All projects" data-testid="project-rail-home">
            <House />
          </RouterLink>
        </TooltipTrigger>
        <TooltipContent side="right">All projects</TooltipContent>
      </Tooltip>

      <Tooltip v-for="tab in tabs" :key="tab.id">
        <TooltipTrigger as-child>
          <RouterLink
            :to="projectPath(props.projectId, tab.id)"
            :class="[item, active === tab.id && 'bg-accent text-foreground']"
            :aria-label="tab.label"
            :aria-current="active === tab.id ? 'page' : undefined"
            :data-testid="`workspace-tab-${tab.id}`"
          >
            <span
              v-if="active === tab.id"
              class="absolute top-1.5 bottom-1.5 -left-2.5 w-1 rounded-r-full bg-primary"
              aria-hidden="true"
            />
            <component :is="tab.icon" />
          </RouterLink>
        </TooltipTrigger>
        <TooltipContent side="right">{{ tab.label }}</TooltipContent>
      </Tooltip>

      <div class="mt-auto flex flex-col items-center gap-1">
        <slot name="footer" />
      </div>
    </nav>
  </TooltipProvider>
</template>
