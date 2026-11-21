<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { TooltipProvider } from "reka-ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import { isMacDesktop } from "@/lib/desktop";
import { cn } from "@/lib/utils";
import { loadTabOrder, saveTabOrder, tabOrderChanged } from "@/lib/tabOrder";
import { useInbox } from "@/modules/review/queries";
import { projectPath, projectTabFromRoute, type ProjectTab } from "@/router";

const props = defineProps<{ projectId: string }>();

const route = useRoute();
// The Review badge counts changes waiting on you; the shared inbox query keeps it fresh.
const review = useInbox(() => props.projectId);
const active = computed(() => projectTabFromRoute(route.name));

/** `icon` is an Iconify Lucide name. */
const defaultTabs: { id: ProjectTab; label: string; icon: string }[] = [
  { id: "preview", label: "Preview", icon: "i-hugeicons-computer-video" },
  { id: "content", label: "Content", icon: "i-hugeicons-file-01" },
  { id: "design", label: "Design", icon: "i-hugeicons-paint-board" },
  { id: "code", label: "Code", icon: "i-hugeicons-source-code" },
  { id: "history", label: "History", icon: "i-hugeicons-clock-04" },
  { id: "review", label: "Review", icon: "i-hugeicons-task-done-01" },
  { id: "settings", label: "Settings", icon: "i-hugeicons-settings-01" },
];

// Tabs can be dragged (or moved with Alt+Up/Down) into the viewer's own order; see
// lib/tabOrder.ts, which also makes opening a project land on the first tab.
const order = ref<ProjectTab[]>(loadTabOrder());
// The daemon's saved order arrived after this rail was drawn (a fresh desktop launch).
watch(tabOrderChanged, () => {
  if (dragging.value === null) order.value = loadTabOrder();
});
const byId = new Map(defaultTabs.map((t) => [t.id, t]));
const tabs = computed(() => order.value.map((id) => byId.get(id)!));

function move(id: ProjectTab, to: number) {
  const ids = tabs.value.map((t) => t.id);
  const from = ids.indexOf(id);
  if (from < 0 || to < 0 || to >= ids.length || from === to) return;
  ids.splice(to, 0, ...ids.splice(from, 1));
  order.value = ids;
}

/** The tab being dragged, and the order to restore if the drag is cancelled. */
const dragging = ref<ProjectTab | null>(null);
let before: ProjectTab[] = [];
const DRAG_TYPE = "application/x-orso-rail-tab";

function onDragStart(event: DragEvent, id: ProjectTab) {
  dragging.value = id;
  before = order.value;
  event.dataTransfer?.setData(DRAG_TYPE, id);
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
}

/** Reorder live as the dragged tab passes over another, so the drop spot is always visible. */
function onDragOver(event: DragEvent, index: number) {
  if (!dragging.value) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  move(dragging.value, index);
}

function onDragEnd(event: DragEvent) {
  if (event.dataTransfer?.dropEffect === "none") order.value = before;
  else saveTabOrder(order.value);
  dragging.value = null;
}

function onKeyMove(event: KeyboardEvent, id: ProjectTab, delta: number) {
  if (!event.altKey) return;
  event.preventDefault();
  const index = tabs.value.findIndex((t) => t.id === id);
  move(id, index + delta);
  saveTabOrder(order.value);
  // The tab moved in the DOM; keep focus on it.
  requestAnimationFrame(() =>
    (document.querySelector(`[data-testid="workspace-tab-${id}"]`) as HTMLElement | null)?.focus(),
  );
}

const onSettings = computed(() => route.path === "/settings");

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

      <div
        v-for="(tab, index) in tabs"
        :key="tab.id"
        draggable="true"
        :class="cn('cursor-grab active:cursor-grabbing', dragging === tab.id && 'opacity-40')"
        :data-testid="`rail-tab-item-${tab.id}`"
        @dragstart="onDragStart($event, tab.id)"
        @dragover="onDragOver($event, index)"
        @drop.prevent
        @dragend="onDragEnd"
      >
      <UTooltip :text="tab.label" :content="tooltipContent" :disabled="dragging !== null">
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
          aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown"
          draggable="false"
          @keydown.up="onKeyMove($event, tab.id, -1)"
          @keydown.down="onKeyMove($event, tab.id, 1)"
        >
          <!-- Review counts the changes waiting on you. -->
          <span
            v-if="tab.id === 'review' && review.waitingCount > 0"
            class="bg-primary text-inverted absolute -top-0.5 -right-0.5 flex min-w-4 items-center justify-center rounded-full px-1 text-[10px] leading-4 font-medium"
            data-testid="review-badge"
          >{{ review.waitingCount }}</span>
        </UButton>
      </UTooltip>
      </div>

      <div class="mt-auto flex flex-col items-center gap-1">
        <UTooltip text="App settings" :content="tooltipContent">
          <UButton
            to="/settings"
            color="neutral"
            variant="ghost"
            square
            icon="i-hugeicons-settings-02"
            active-color="primary"
            active-variant="soft"
            :active="onSettings"
            :ui="ui"
            :class="cn(onSettings && current)"
            :aria-current="onSettings ? 'page' : undefined"
            aria-label="App settings"
            data-testid="project-rail-app-settings"
          />
        </UTooltip>
        <slot name="footer" />
      </div>
    </nav>
  </TooltipProvider>
</template>
