<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import {
  ArrowLeft,
  ArrowRight,
  ExternalLink,
  Maximize2,
  Minimize2,
  Monitor,
  MousePointerClick,
  RotateCw,
  Smartphone,
  Tablet,
} from "@lucide/vue";
import { useEventListener } from "@vueuse/core";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { isCrossSite } from "@/core/config";
import { useDaemon } from "@/core/daemon";
import type { DevserverState } from "@/core/types";
import PrStatusPill from "@/modules/changes/PrStatusPill.vue";
import { attachPicker, describeElement } from "./elementPicker";
import { frameHref, framePath, useFrameHistory } from "./frameHistory";
import { DEVICE_WIDTH, type Device, usePreviewStore } from "./store";

const props = defineProps<{ projectId: string; devserver?: DevserverState }>();

const store = usePreviewStore();
const daemon = useDaemon();

watch(
  () => [props.projectId, props.devserver] as const,
  ([id, state]) => {
    if (state) store.seed(id, state);
  },
  { immediate: true },
);

const state = computed<DevserverState>(
  () => store.status[props.projectId] ?? props.devserver ?? "starting",
);
const src = computed(() => daemon.api.previewUrl(props.projectId));
const base = computed(() => new URL(src.value, window.location.href).pathname);
// A cross-site daemon cannot authenticate the framed preview (SameSite=Lax `wb_session`); see
// `previewUrl` in core/api.ts. Explain instead of showing a wall of 401s; the user may override
// (e.g. sibling subdomains are same-site but look cross-site to us).
const forceFrame = ref(false);
const daemonOrigin = computed(() => daemon.config.value.origin);
const crossSite = computed(
  () => !forceFrame.value && isCrossSite(window.location.origin, daemonOrigin.value),
);
const width = computed(() => DEVICE_WIDTH[store.device]);

const DEVICES: { id: Device; label: string; icon: typeof Monitor }[] = [
  { id: "desktop", label: "Desktop", icon: Monitor },
  { id: "tablet", label: "Tablet", icon: Tablet },
  { id: "mobile", label: "Mobile", icon: Smartphone },
];
const device = computed(() => DEVICES.find((d) => d.id === store.device) ?? DEVICES[0]!);

const DOT: Record<DevserverState, string> = {
  starting: "bg-amber-500 animate-pulse",
  ready: "bg-emerald-500",
  crashed: "bg-red-500",
};
const LABEL: Record<DevserverState, string> = {
  starting: "Dev server starting",
  ready: "Dev server ready",
  crashed: "Dev server crashed",
};

// --- Browser shell ---------------------------------------------------------------------------

const shell = ref<HTMLElement | null>(null);
const frame = ref<HTMLIFrameElement | null>(null);
const history = useFrameHistory();
/** Where the frame starts when (re)mounted: a reload keeps the page the user was on. */
const startHref = ref(src.value);
const draft = ref<string | null>(null);

watch(
  src,
  (url) => {
    history.reset();
    startHref.value = url;
  },
);

watch(
  () => store.reloadNonce,
  () => {
    if (history.entries.value.length) {
      startHref.value = new URL(frameHref(history.current.value, base.value), src.value).href;
    }
  },
);

/** The frame's window when it is same-origin (the normal, proxied setup); null otherwise. */
function frameWindow(): Window | null {
  const win = frame.value?.contentWindow;
  try {
    return win && win.location.href ? win : null;
  } catch {
    return null;
  }
}

function onLoad() {
  const win = frameWindow();
  syncPicker();
  if (!win) return;
  const sync = () => history.visit(framePath(win.location.href, base.value));
  sync();
  // Same-document navigations: history traversal, anchors, and Astro's client-side router.
  win.addEventListener("popstate", sync);
  win.addEventListener("hashchange", sync);
  win.document.addEventListener("astro:page-load", sync);
}

// --- Element picker ----------------------------------------------------------------------------

let detachPicker: (() => void) | null = null;

/** Attach to (or detach from) the current frame document to match `store.picking`. */
function syncPicker() {
  detachPicker?.();
  detachPicker = null;
  if (!store.picking) return;
  const win = frameWindow();
  // Cross-origin or not yet loaded: nothing to pick from.
  if (!win || state.value !== "ready") {
    store.setPicking(false);
    return;
  }
  detachPicker = attachPicker(
    win,
    (el) => store.pick(describeElement(el, props.projectId)),
    () => store.setPicking(false),
  );
}

watch(() => store.picking, syncPicker);
// A remount (reload, project switch) replaces the document; `onLoad` re-attaches.
watch(() => [props.projectId, store.reloadNonce, state.value], () => {
  detachPicker = null;
  if (state.value !== "ready") store.setPicking(false);
});
onBeforeUnmount(() => {
  detachPicker?.();
  store.setPicking(false);
});

function goBack() {
  if (history.back()) frameWindow()?.history.back();
}

function goForward() {
  if (history.forward()) frameWindow()?.history.forward();
}

function navigate() {
  if (draft.value === null) return;
  const href = frameHref(draft.value, base.value);
  draft.value = null;
  const win = frameWindow();
  if (win) win.location.assign(href);
  else if (frame.value) frame.value.src = new URL(href, src.value).href;
}

function openExternal() {
  const href = new URL(frameHref(history.current.value, base.value), src.value).href;
  window.open(href, "_blank", "noopener");
}

// Tracks Esc / browser-initiated exits too, so the button always offers the right action.
const isFullscreen = ref(false);
useEventListener(document, "fullscreenchange", () => {
  isFullscreen.value = !!shell.value && document.fullscreenElement === shell.value;
});

function toggleFullscreen() {
  if (isFullscreen.value) void document.exitFullscreen?.();
  else void shell.value?.requestFullscreen?.();
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="preview-panel">
    <div ref="shell" class="bg-background flex min-h-0 flex-1 flex-col overflow-hidden">
      <TooltipProvider>
        <div class="bg-muted/40 flex h-10 shrink-0 items-center gap-0.5 border-b px-1.5">
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Back"
                :disabled="!history.canGoBack.value"
                @click="goBack"
              >
                <ArrowLeft />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Back</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Forward"
                :disabled="!history.canGoForward.value"
                @click="goForward"
              >
                <ArrowRight />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Forward</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button variant="ghost" size="icon-sm" aria-label="Reload preview" @click="store.reload()">
                <RotateCw />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Reload</TooltipContent>
          </Tooltip>

          <form class="mx-1 flex min-w-0 flex-1" @submit.prevent="navigate">
            <div
              class="bg-background focus-within:border-ring flex h-7 min-w-0 flex-1 items-center gap-2 rounded-md border px-2.5"
            >
              <Tooltip>
                <TooltipTrigger as-child>
                  <span
                    class="size-1.5 shrink-0 rounded-full"
                    :class="DOT[state]"
                    :data-state="state"
                    role="status"
                    :aria-label="LABEL[state]"
                    data-testid="status-dot"
                  />
                </TooltipTrigger>
                <TooltipContent>{{ LABEL[state] }}</TooltipContent>
              </Tooltip>
              <input
                :value="draft ?? history.current.value"
                class="min-w-0 flex-1 bg-transparent text-sm outline-none"
                aria-label="Preview address"
                spellcheck="false"
                autocomplete="off"
                data-testid="preview-address"
                @input="draft = ($event.target as HTMLInputElement).value"
                @focus="($event.target as HTMLInputElement).select()"
                @blur="draft = null"
                @keydown.esc="draft = null; ($event.target as HTMLInputElement).blur()"
              />
            </div>
          </form>

          <PrStatusPill />

          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button
                variant="ghost"
                size="icon-sm"
                :aria-label="`Viewport: ${device.label}`"
                data-testid="viewport-size"
              >
                <component :is="device.icon" />
              </Button>
            </DropdownMenuTrigger>
            <!-- Portal into the shell while it is fullscreen; `body` is not rendered then. -->
            <DropdownMenuContent align="end" side="bottom" :to="isFullscreen ? shell ?? undefined : undefined">
              <DropdownMenuRadioGroup
                :model-value="store.device"
                @update:model-value="store.setDevice($event as Device)"
              >
                <DropdownMenuRadioItem v-for="d in DEVICES" :key="d.id" :value="d.id">
                  <component :is="d.icon" />
                  {{ d.label }}
                  <span v-if="DEVICE_WIDTH[d.id]" class="text-muted-foreground ml-auto pl-3 text-xs">
                    {{ DEVICE_WIDTH[d.id] }}px
                  </span>
                </DropdownMenuRadioItem>
              </DropdownMenuRadioGroup>
            </DropdownMenuContent>
          </DropdownMenu>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                :variant="store.picking ? 'secondary' : 'ghost'"
                size="icon-sm"
                aria-label="Select an element"
                :aria-pressed="store.picking"
                :disabled="state !== 'ready' || crossSite"
                data-testid="element-picker"
                @click="store.setPicking(!store.picking)"
              >
                <MousePointerClick />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {{ store.picking ? "Cancel selection (Esc)" : "Select an element to mention in chat" }}
            </TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button variant="ghost" size="icon-sm" aria-label="Open in new tab" @click="openExternal">
                <ExternalLink />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Open in new tab</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                size="icon-sm"
                :aria-label="isFullscreen ? 'Exit full screen' : 'Full screen'"
                @click="toggleFullscreen"
              >
                <Minimize2 v-if="isFullscreen" />
                <Maximize2 v-else />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{{ isFullscreen ? "Exit full screen" : "Full screen" }}</TooltipContent>
          </Tooltip>
        </div>
      </TooltipProvider>

      <div class="bg-muted/30 flex min-h-0 flex-1 justify-center overflow-auto">
        <div
          v-if="state === 'crashed'"
          class="flex flex-col items-center justify-center gap-3 p-6 text-center text-sm"
          role="alert"
        >
          <p>The dev server crashed{{ store.message[projectId] ? `: ${store.message[projectId]}` : "." }}</p>
          <p v-if="store.error" class="text-destructive">{{ store.error }}</p>
          <Button :disabled="store.restarting" @click="store.restart(projectId)">
            {{ store.restarting ? "Restarting…" : "Restart dev server" }}
          </Button>
        </div>
        <div
          v-else-if="state === 'starting'"
          class="text-muted-foreground flex items-center justify-center text-sm"
        >
          Starting dev server…
        </div>
        <div
          v-else-if="crossSite"
          class="flex max-w-md flex-col items-center justify-center gap-3 p-6 text-center text-sm"
          role="note"
          data-testid="preview-cross-site"
        >
          <p class="font-medium">Live preview isn't available from this page</p>
          <p class="text-muted-foreground">
            The daemon at {{ daemonOrigin }} is on a different site than this app, so the
            browser won't send its session cookie to the preview frame. Open the daemon link
            directly (the one printed at startup) to use the live preview.
          </p>
          <Button variant="outline" size="sm" @click="forceFrame = true">Try anyway</Button>
        </div>
        <iframe
          v-else
          ref="frame"
          :key="`${projectId}:${store.reloadNonce}`"
          :src="startHref"
          title="Site preview"
          class="h-full border-0 bg-white"
          :style="{ width: width ? `${width}px` : '100%' }"
          data-testid="preview-frame"
          @load="onLoad"
        />
      </div>
    </div>
  </div>
</template>
