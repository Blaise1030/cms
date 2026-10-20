<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import {
  ArrowLeft,
  ArrowRight,
  ExternalLink,
  Maximize2,
  Minimize2,
  Monitor,
  Moon,
  MousePointerClick,
  PilcrowRight,
  RotateCw,
  Smartphone,
  Sparkles,
  Sun,
  Tablet,
  TriangleAlert,
  X,
} from "@lucide/vue";
import { useElementSize, useEventListener } from "@vueuse/core";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { isCrossSite } from "@/core/config";
import { useDaemon } from "@/core/daemon";
import type { DevserverState } from "@/core/types";
import CleanupChangeButton from "@/modules/changes/CleanupChangeButton.vue";
import { frameHref, framePath, readPref, writePref } from "@orso/utils";
import { attachPicker, describeElement } from "./elementPicker";
import { captureElement } from "./elementShot";
import { applyFrameAppearance } from "./frameAppearance";
import { DESKTOP_MIN_WIDTH, frameFit } from "./frameFit";
import { useFrameHistory } from "./frameHistory";
import { type PageError, watchPageErrors } from "./pageErrors";
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
/** The area the frame sits in: the page is laid out at the device width and scaled to fit it. */
const viewport = ref<HTMLElement | null>(null);
const viewportSize = useElementSize(viewport);
const fit = computed(() =>
  frameFit(store.device, viewportSize.width.value, viewportSize.height.value),
);
const zoomed = computed(() => fit.value.scale < 1);
const zoomPercent = computed(() => Math.round(fit.value.scale * 100));

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
/** The last path the frame showed, per project, so the preview reopens where the user left it. */
const lastPathKey = (projectId: string) => `orso.preview.path.${projectId}`;
const lastPath = ref(readPref(lastPathKey(props.projectId)) ?? "/");
/** What the address bar shows: the frame's page, or the restored one until the frame reports in. */
const currentPath = computed(() =>
  history.entries.value.length ? history.current.value : lastPath.value,
);
const hrefFor = (path: string) => new URL(frameHref(path, base.value), src.value).href;
/** Where the frame starts when (re)mounted: a reload keeps the page the user was on. */
const startHref = ref(hrefFor(lastPath.value));
const draft = ref<string | null>(null);

watch(
  src,
  () => {
    history.reset();
    lastPath.value = readPref(lastPathKey(props.projectId)) ?? "/";
    startHref.value = hrefFor(lastPath.value);
  },
);

watch(
  () => history.entries.value.length && history.current.value,
  (path) => {
    if (path) writePref(lastPathKey(props.projectId), path);
  },
);

watch(
  () => store.reloadNonce,
  () => {
    if (history.entries.value.length) {
      startHref.value = hrefFor(history.current.value);
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
  watchErrors(win);
  if (!win) return;
  const sync = () => history.visit(framePath(win.location.href, base.value));
  sync();
  // Same-document navigations: history traversal, anchors, and Astro's client-side router.
  win.addEventListener("popstate", sync);
  win.addEventListener("hashchange", sync);
  win.document.addEventListener("astro:page-load", sync);
  syncAppearance();
  // Astro's router copies the next page's `<html>` attributes over ours.
  win.document.addEventListener("astro:after-swap", () => syncAppearance(true));
}

// --- Page errors -------------------------------------------------------------------------------

/** The error to offer a fix for: an error overlay wins over uncaught errors. */
const pageError = ref<PageError | null>(null);
let detachErrors: (() => void) | null = null;

/** Follow the newly loaded document; the previous one's errors went with it. */
function watchErrors(win: Window | null) {
  detachErrors?.();
  detachErrors = null;
  pageError.value = null;
  if (!win) return;
  detachErrors = watchPageErrors(win, {
    onError(error) {
      if (error.kind === "build" || pageError.value?.kind !== "build") pageError.value = error;
    },
    onOverlayCleared() {
      if (pageError.value?.kind === "build") pageError.value = null;
    },
  });
}

onBeforeUnmount(() => detachErrors?.());

function fixPageError() {
  if (!pageError.value) return;
  store.fixPageError(pageError.value, currentPath.value);
  pageError.value = null;
}

// --- RTL / dark mode -----------------------------------------------------------------------------

function syncAppearance(fresh = false) {
  const doc = frameWindow()?.document;
  if (doc) applyFrameAppearance(doc, { rtl: store.rtl, dark: store.dark }, fresh);
}

watch(() => [store.rtl, store.dark], () => syncAppearance());

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
    (el) => void pickElement(el),
    () => store.setPicking(false),
  );
}

/** Mention a picked element, with a screenshot of it when one can be taken. */
async function pickElement(el: Element) {
  const projectId = props.projectId;
  const element = describeElement(el, projectId);
  store.setPicking(false);
  try {
    const shot = await captureElement(el);
    if (shot) element.screenshot = await daemon.api.attachTerminalImage(projectId, shot);
  } catch (e) {
    // The mention is still useful without the picture.
    console.warn("element screenshot failed", e);
  }
  store.pick(element);
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
  const href = hrefFor(currentPath.value);
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
  <div class="flex h-full min-h-0 flex-col pb-2" data-testid="preview-panel">
    <div ref="shell" class="bg-background flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border shadow-sm">
      <TooltipProvider>
        <div class="bg-background flex h-10 shrink-0 items-center gap-0.5 border-b px-1.5">
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
              class="bg-card focus-within:border-ring flex h-7 min-w-0 flex-1 items-center gap-2 rounded-md border px-2.5"
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
                :value="draft ?? currentPath"
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

          <CleanupChangeButton :project-id="projectId" />

          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button
                variant="ghost"
                :size="zoomed ? 'sm' : 'icon-sm'"
                :aria-label="`Viewport: ${device.label}${zoomed ? `, ${zoomPercent}%` : ''}`"
                data-testid="viewport-size"
              >
                <component :is="device.icon" />
                <span v-if="zoomed" class="text-muted-foreground text-xs tabular-nums">{{ zoomPercent }}%</span>
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
                  <span class="text-muted-foreground ml-auto pl-3 text-xs">
                    {{ DEVICE_WIDTH[d.id] ? `${DEVICE_WIDTH[d.id]}px` : `${DESKTOP_MIN_WIDTH}px+` }}
                  </span>
                </DropdownMenuRadioItem>
              </DropdownMenuRadioGroup>
            </DropdownMenuContent>
          </DropdownMenu>
          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button
                :variant="store.rtl || store.dark ? 'secondary' : 'ghost'"
                size="icon-sm"
                aria-label="Appearance"
                data-testid="preview-appearance"
              >
                <Moon v-if="store.dark" />
                <Sun v-else />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" side="bottom" :to="isFullscreen ? shell ?? undefined : undefined">
              <DropdownMenuRadioGroup
                :model-value="store.dark ? 'dark' : 'light'"
                @update:model-value="($event === 'dark') !== store.dark && store.toggleDark()"
              >
                <DropdownMenuRadioItem value="light" data-testid="preview-light">
                  <Sun />
                  Light
                </DropdownMenuRadioItem>
                <DropdownMenuRadioItem value="dark" data-testid="preview-dark">
                  <Moon />
                  Dark
                </DropdownMenuRadioItem>
              </DropdownMenuRadioGroup>
              <DropdownMenuSeparator />
              <DropdownMenuCheckboxItem
                :model-value="store.rtl"
                data-testid="preview-rtl"
                @update:model-value="store.toggleRtl()"
              >
                <PilcrowRight />
                Right-to-left
              </DropdownMenuCheckboxItem>
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
              {{ store.picking ? "Cancel selection (Esc)" : "Select an element to mention in the chat" }}
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

      <div class="relative flex min-h-0 flex-1 flex-col">
      <div ref="viewport" class="bg-muted/30 flex min-h-0 flex-1 justify-center overflow-auto">
        <div
          v-if="state === 'crashed'"
          class="flex flex-col items-center justify-center gap-3 p-6 text-center text-sm"
          role="alert"
        >
          <p>The dev server crashed{{ store.message[projectId] ? `: ${store.message[projectId]}` : "." }}</p>
          <p v-if="store.error" class="text-destructive">{{ store.error }}</p>
          <div class="flex gap-2">
            <Button :disabled="store.restarting" @click="store.restart(projectId)">
              {{ store.restarting ? "Restarting…" : "Restart dev server" }}
            </Button>
            <Button variant="outline" data-testid="fix-crash" @click="store.fixCrash(projectId)">
              <Sparkles />
              Fix with agent
            </Button>
          </div>
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
        <!-- Sized to the scaled frame, so the panel centres and clips what is actually shown. -->
        <div
          v-else
          class="h-full shrink-0 overflow-hidden"
          :style="zoomed ? { width: `${fit.width! * fit.scale}px` } : { width: fit.width ? `${fit.width}px` : '100%' }"
        >
          <iframe
            ref="frame"
            :key="`${projectId}:${store.reloadNonce}`"
            :src="startHref"
            title="Site preview"
            class="h-full origin-top-left border-0"
            :class="store.dark ? 'bg-neutral-950' : 'bg-white'"
            :style="{
              width: fit.width ? `${fit.width}px` : '100%',
              ...(zoomed ? { height: `${fit.height}px`, transform: `scale(${fit.scale})` } : {}),
              // An iframe's `color-scheme` sets `prefers-color-scheme` inside the framed page.
              colorScheme: store.dark ? 'dark' : 'light',
            }"
            data-testid="preview-frame"
            @load="onLoad"
          />
        </div>
      </div>

      <div
        v-if="pageError && state === 'ready'"
        class="bg-background absolute inset-x-3 bottom-3 flex items-start gap-3 rounded-lg border p-3 text-sm shadow-lg"
        role="alert"
        data-testid="page-error"
      >
        <TriangleAlert class="text-destructive mt-0.5 size-4 shrink-0" />
        <div class="min-w-0 flex-1">
          <p class="font-medium">
            {{ pageError.kind === "build" ? "This page failed to build" : "This page threw an error" }}
          </p>
          <p class="text-muted-foreground line-clamp-2 font-mono text-xs break-words">{{ pageError.message }}</p>
        </div>
        <Button size="sm" data-testid="fix-page-error" @click="fixPageError">
          <Sparkles />
          Fix with agent
        </Button>
        <Button variant="ghost" size="icon-sm" aria-label="Dismiss error" @click="pageError = null">
          <X />
        </Button>
      </div>
      </div>
    </div>
  </div>
</template>
