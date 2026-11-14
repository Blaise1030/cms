<script setup lang="ts">
import { computed, type Component } from "vue";
import { RouterLink, useRoute } from "vue-router";
import { Code, Cog, FileText, History, House, MonitorPlay, Palette, Settings } from "@lucide/vue";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { isMacDesktop } from "@/lib/desktop";
import { cn } from "@/lib/utils";
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
  { id: "settings", label: "Settings", icon: Settings },
];

// On desktop the rail starts at the top bar's height (pt-11), below the traffic lights, except
// in full screen where they are hidden.
const macDesktop = isMacDesktop();

const item = "relative flex size-9 items-center justify-center rounded-lg text-muted-foreground outline-none transition-all active:translate-y-px hover:bg-background hover:text-foreground hover:btn-raised-soft dark:hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring [&_svg]:size-5";
/** The home link sits raised, like the app's other buttons. */
const raised = "bg-background text-foreground btn-raised-soft dark:bg-accent/50";
/** Selected tab: primary tint + raised gloss. */
const current =
  "bg-primary/12 text-primary btn-raised-soft hover:bg-primary/16 hover:text-primary dark:bg-primary/25 dark:text-primary-foreground dark:hover:bg-primary/30";
</script>

<template>
  <TooltipProvider :delay-duration="300">
    <nav
      :class="['flex h-full w-14 shrink-0 flex-col items-center gap-1 bg-background pb-2', macDesktop ? 'pt-11 fullscreen:pt-2' : 'pt-2']"
      aria-label="Project"
      :data-tauri-drag-region="macDesktop ? 'deep' : undefined"
      data-testid="project-rail"
    >
      <Tooltip>
        <TooltipTrigger as-child>
          <RouterLink to="/" :class="[item, raised, 'mb-2']" aria-label="All projects" data-testid="project-rail-home">
            <House />
          </RouterLink>
        </TooltipTrigger>
        <TooltipContent side="right">All projects</TooltipContent>
      </Tooltip>

      <Tooltip v-for="tab in tabs" :key="tab.id">
        <TooltipTrigger as-child>
          <RouterLink
            :to="projectPath(props.projectId, tab.id)"
            :class="cn(item, active === tab.id && current)"
            :aria-label="tab.label"
            :aria-current="active === tab.id ? 'page' : undefined"
            :data-testid="`workspace-tab-${tab.id}`"
          >
            <component :is="tab.icon" />
          </RouterLink>
        </TooltipTrigger>
        <TooltipContent side="right">{{ tab.label }}</TooltipContent>
      </Tooltip>

      <div class="mt-auto flex flex-col items-center gap-1">
        <Tooltip>
          <TooltipTrigger as-child>
            <RouterLink
              to="/settings"
              :class="item"
              aria-label="App settings"
              data-testid="project-rail-app-settings"
            >
              <Cog />
            </RouterLink>
          </TooltipTrigger>
          <TooltipContent side="right">App settings</TooltipContent>
        </Tooltip>
        <slot name="footer" />
      </div>
    </nav>
  </TooltipProvider>
</template>
