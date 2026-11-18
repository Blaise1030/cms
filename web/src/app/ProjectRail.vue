<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import { TooltipProvider } from "reka-ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import { isMacDesktop } from "@/lib/desktop";
import { cn } from "@/lib/utils";
import { useReviewRefresh } from "@/modules/review/useReviewRefresh";
import { useReviewStore } from "@/modules/review/store";
import { projectPath, projectTabFromRoute, type ProjectTab } from "@/router";

const props = defineProps<{ projectId: string }>();

const route = useRoute();
const review = useReviewStore();
// The Review badge counts changes waiting on you, so the rail keeps the inbox fresh.
useReviewRefresh(() => void review.load(props.projectId), () => props.projectId);
const active = computed(() => projectTabFromRoute(route.name));

/** `icon` is an Iconify Lucide name. */
const tabs: { id: ProjectTab; label: string; icon: string }[] = [
  { id: "preview", label: "Preview", icon: "i-hugeicons-computer-video" },
  { id: "content", label: "Content", icon: "i-hugeicons-file-01" },
  { id: "design", label: "Design", icon: "i-hugeicons-paint-board" },
  { id: "code", label: "Code", icon: "i-hugeicons-source-code" },
  { id: "history", label: "History", icon: "i-hugeicons-clock-04" },
  { id: "review", label: "Review", icon: "i-hugeicons-task-done-01" },
  { id: "settings", label: "Settings", icon: "i-hugeicons-settings-01" },
];

// On desktop the rail starts at the top bar's height (pt-11), below the traffic lights, except
// in full screen where they are hidden.
const macDesktop = isMacDesktop();

/** Rail links: 36px square icon buttons. */
const ui = { base: "size-9 justify-center rounded-lg", leadingIcon: "size-5" };
/** Selected tab: primary tint. */
const current = "dark:bg-primary/25 dark:text-highlighted dark:hover:bg-primary/30";
const tooltipContent = { side: "right" } as const;
</script>

<template>
  <TooltipProvider :delay-duration="300">
    <nav
      :class="['flex h-full w-14 shrink-0 flex-col items-center gap-1 bg-background pb-2', macDesktop ? 'pt-11 fullscreen:pt-2' : 'pt-2']"
      aria-label="Project"
      :data-tauri-drag-region="macDesktop ? 'deep' : undefined"
      data-testid="project-rail"
    >
      <!-- The home link sits raised, like the app's other buttons. -->
      <UTooltip text="All projects" :content="tooltipContent">
        <UButton
          to="/"
          color="neutral"
          variant="outline"
          square
          icon="i-hugeicons-home-01"
          :active="false"
          :ui="ui"
          class="mb-2 shadow-xs"
          aria-label="All projects"
          data-testid="project-rail-home"
        />
      </UTooltip>

      <UTooltip v-for="tab in tabs" :key="tab.id" :text="tab.label" :content="tooltipContent">
        <UButton
          :to="projectPath(props.projectId, tab.id)"
          color="neutral"
          variant="ghost"
          active-color="primary"
          active-variant="soft"
          square
          :icon="tab.icon"
          :active="active === tab.id"
          :class="cn(active === tab.id && current)"
          :aria-label="tab.label"
          :aria-current="active === tab.id ? 'page' : undefined"
          :data-testid="`workspace-tab-${tab.id}`"
          :ui="tab.id === 'review' ? { ...ui, base: cn(ui.base, 'relative') } : ui"
        >
          <span
            v-if="tab.id === 'review' && review.waitingCount > 0"
            class="bg-primary text-inverted absolute -top-0.5 -right-0.5 flex min-w-4 items-center justify-center rounded-full px-1 text-[10px] leading-4 font-medium"
            data-testid="review-badge"
          >{{ review.waitingCount }}</span>
        </UButton>
      </UTooltip>

      <div class="mt-auto flex flex-col items-center gap-1">
        <UTooltip text="App settings" :content="tooltipContent">
          <UButton
            to="/settings"
            color="neutral"
            variant="ghost"
            square
            icon="i-hugeicons-settings-02"
            :active="false"
            :ui="ui"
            aria-label="App settings"
            data-testid="project-rail-app-settings"
          />
        </UTooltip>
        <slot name="footer" />
      </div>
    </nav>
  </TooltipProvider>
</template>
