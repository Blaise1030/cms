<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useElementSize, useEventListener } from "@vueuse/core";
import type { DropdownMenuItem, SplitterItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UChip from "@nuxt/ui/components/Chip.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import USplitter from "@nuxt/ui/components/Splitter.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import { isCrossSite } from "@/core/config";
import { useDaemon } from "@/core/daemon";
import { type PreviewAccess, previewHref, previewOrigin } from "@/core/previewOrigin";
import type { DevserverState } from "@/core/types";
import CleanupChangeButton from "@/modules/changes/CleanupChangeButton.vue";
import { framePath } from "@orso/utils";
import { type PickedElement, relativeFile } from "./elementPicker";
import { BRIDGE_TYPE, dataUrlToBlob, isBridgeMessage } from "./frameBridge";
import { useChatPanelLayout } from "@/lib/chatPanelLayout";
import { cn } from "@/lib/utils";
import { auditHtml, type PageAudit } from "./checks/auditDocument";
import type { Finding } from "./checks/findings";
import type { PageMeta } from "./checks/meta";
import type { ResourceSummary } from "./checks/resourceSummary";
import { DESKTOP_MIN_WIDTH, frameFit } from "./frameFit";
import { useFrameHistory } from "./frameHistory";
import PageDetailsSidebar from "./PageDetailsSidebar.vue";
import type { PageError } from "./pageErrors";
import { DEVICE_WIDTH, type Device, usePreviewStore } from "./store";

const props = defineProps<{ projectId: string; devserver?: DevserverState }>();

const store = usePreviewStore();
const panelLayout = useChatPanelLayout();
/** Floating agent chrome sits under the preview, so the shell gets a full radius again. */
const shellClass = computed(() =>
  cn(
    "bg-background flex min-h-0 flex-1 flex-col overflow-hidden border border-default shadow-sm",
    panelLayout.floating.value ? "rounded-xl" : "rounded-t-xl border-b-0",
  ),
);
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

/**
 * The daemon serves the preview at the root of an origin of its own (a loopback port that
 * proxies the dev server), so the site runs with base `/` as in production. The frame is
 * therefore always cross-origin: everything the panel does inside it (picking, appearance,
 * address, errors, Page details) goes through the bridge the orso Astro integration injects
 * into every dev page (see `packages/astro/src/bridge.js`), and messages are exchanged with
 * that origin only.
 */
const access = ref<PreviewAccess | null>(null);
const accessError = ref<string | null>(null);
let accessRun = 0;

async function loadAccess() {
  const run = ++accessRun;
  try {
    const next = await daemon.api.previewAccess(props.projectId);
    if (run !== accessRun) return;
    accessError.value = null;
    // Keep the object when nothing changed, so the frame is not remounted.
    if (next.port !== access.value?.port || next.token !== access.value?.token) access.value = next;
  } catch (e) {
    if (run !== accessRun) return;
    accessError.value = e instanceof Error ? e.message : String(e);
  }
}

watch(
  () => props.projectId,
  () => {
    access.value = null;
    void loadAccess();
  },
  { immediate: true },
);
// A restart can follow a project being removed and re-added; ask again once it is up.
watch(state, (now, before) => {
  if (now === "ready" && before !== "ready") void loadAccess();
});

const daemonOrigin = computed(() => daemon.config.value.origin);
/** The framed page's origin: the only one the panel posts to or accepts messages from. */
const frameOrigin = computed(() => (access.value ? previewOrigin(access.value, daemonOrigin.value) : null));
// The preview origin's cookie is `SameSite=Lax`: a page on another site than the daemon (hosted
// PWA at `app.<domain>` driving a loopback daemon) can frame it, but the browser neither stores
// nor sends that cookie there, so every request inside the frame 401s. Explain instead of
// showing a wall of 401s; the user may override (sibling subdomains are same-site but look
// cross-site to us).
const forceFrame = ref(false);
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

const DEVICES: { id: Device; label: string; icon: string }[] = [
  { id: "desktop", label: "Desktop", icon: "i-hugeicons-computer" },
  { id: "tablet", label: "Tablet", icon: "i-hugeicons-tablet-01" },
  { id: "mobile", label: "Mobile", icon: "i-hugeicons-smart-phone-01" },
];
const device = computed(() => DEVICES.find((d) => d.id === store.device) ?? DEVICES[0]!);

const deviceItems = computed<DropdownMenuItem[]>(() =>
  DEVICES.map((d) => ({
    type: "checkbox",
    label: d.label,
    icon: d.icon,
    checked: store.device === d.id,
    width: DEVICE_WIDTH[d.id] ? `${DEVICE_WIDTH[d.id]}px` : `${DESKTOP_MIN_WIDTH}px+`,
    onUpdateChecked: () => store.setDevice(d.id),
  })),
);

/** Light/dark behave as a radio pair; right-to-left is an independent toggle. */
const appearanceItems = computed<DropdownMenuItem[][]>(() => [
  [
    {
      type: "checkbox",
      label: "Light",
      icon: "i-hugeicons-sun-03",
      testid: "preview-light",
      checked: !store.dark,
      onUpdateChecked: () => store.dark && store.toggleDark(),
    },
    {
      type: "checkbox",
      label: "Dark",
      icon: "i-hugeicons-moon-02",
      testid: "preview-dark",
      checked: store.dark,
      onUpdateChecked: () => !store.dark && store.toggleDark(),
    },
  ],
  [
    {
      type: "checkbox",
      label: "Right-to-left",
      icon: "i-hugeicons-paragraph",
      testid: "preview-rtl",
      checked: store.rtl,
      onUpdateChecked: () => store.toggleRtl(),
    },
  ],
]);

const DOT: Record<DevserverState, string> = {
  starting: "bg-warning animate-pulse",
  ready: "bg-success",
  crashed: "bg-error",
};
const LABEL: Record<DevserverState, string> = {
  starting: "Dev server starting",
  ready: "Dev server ready",
  crashed: "Dev server crashed",
};

// --- Browser shell ---------------------------------------------------------------------------

const frame = ref<HTMLIFrameElement | null>(null);
const history = useFrameHistory();
/** The last path the frame showed, per project, so the preview reopens where the user left it. */
const lastPath = computed(() => store.framePathFor(props.projectId));
/** What the address bar shows: the frame's page, or the restored one until the frame reports in. */
const currentPath = computed(() =>
  history.entries.value.length ? history.current.value : lastPath.value,
);
/** The page's address on the preview origin, as the reader would see it. */
const pageHref = (path: string) =>
  access.value ? previewHref(access.value, daemonOrigin.value, path) : "";
/**
 * The address the app loads the frame with: also carries the origin's first-load token, which
 * the origin trades for its own cookie and drops from the address before the page sees it.
 */
const loadHref = (path: string) =>
  access.value ? previewHref(access.value, daemonOrigin.value, path, true) : "";
/** Where the frame starts when (re)mounted: a reload keeps the page the user was on. */
const startHref = ref("");
const draft = ref<string | null>(null);

watch(
  frameOrigin,
  () => {
    history.reset();
    startHref.value = loadHref(lastPath.value);
  },
  { immediate: true },
);

watch(
  () => history.entries.value.length && history.current.value,
  (path) => {
    if (path) store.setFramePath(props.projectId, path);
  },
);

watch(
  () => store.reloadNonce,
  () => {
    // The frame's page is only known from what the bridge reported; without it a reload would
    // return the reader to the address it started at.
    startHref.value = loadHref(currentPath.value);
  },
);

watch(
  () => store.navigateNonce,
  () => {
    // A content "open in preview" must win over the frame's back stack. `setFramePath` alone is
    // also how the bridge reports the page the frame is already on, so it must not navigate.
    history.reset();
    startHref.value = loadHref(lastPath.value);
  },
);

// --- Bridge ------------------------------------------------------------------------------------

/** Drive the framed page over `postMessage`, addressed to its origin only. */
function postToFrame(op: string, extra: Record<string, unknown> = {}) {
  const origin = frameOrigin.value;
  if (!origin) return;
  frame.value?.contentWindow?.postMessage({ type: BRIDGE_TYPE, op, ...extra }, origin);
}

function onFrameMessage(e: MessageEvent) {
  // Only the framed page, and only while it is on the preview origin.
  if (!e.source || e.source !== frame.value?.contentWindow) return;
  if (!frameOrigin.value || e.origin !== frameOrigin.value) return;
  if (!isBridgeMessage(e.data)) return;
  switch (e.data.op) {
    case "ready":
      return;
    case "path": {
      // The frame only reveals its page this way: it feeds the address bar and the reload
      // target, and it is the only way the back/forward stack learns where the reader went.
      const path = framePath(e.data.url, "/");
      store.setFramePath(props.projectId, path);
      history.visit(path);
      // Re-audit after client-side navigations reported by the bridge.
      refreshPageDetails();
      return;
    }
    case "snapshot":
      applyAudit(auditHtml(e.data.html, framePath(e.data.url, "/"), e.data.url));
      return;
    case "cancel":
      store.setPicking(false);
      return;
    case "fix":
      store.fixCheck(e.data.text);
      return;
    case "error":
      onPageError(e.data.error);
      return;
    case "error-cleared":
      if (pageError.value?.kind === "build") pageError.value = null;
      return;
    case "pick":
      void pickElement(e.data.element);
      return;
  }
}

useEventListener(window, "message", onFrameMessage);

/** Esc leaves picking wherever the focus is; the page handles its own copy inside the frame. */
function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && store.picking) store.setPicking(false);
}

useEventListener(window, "keydown", onKeydown);

function onLoad() {
  // The previous document's error went with it.
  pageError.value = null;
  // Introduce the app, so the bridge answers this origin from now on (and replays the page's
  // address and any error it saw before it knew where to send them).
  postToFrame("hello");
  syncPicker();
  syncAppearance();
  refreshPageDetails();
}

// --- Page details (SEO snapshot + findings) ----------------------------------------------------

const detailsOpen = ref(false);
const pageMeta = ref<PageMeta | null>(null);
const pageFindings = ref<Finding[]>([]);
const pageResources = ref<ResourceSummary | null>(null);
const errorCount = computed(
  () => pageFindings.value.filter((f) => f.severity === "error").length,
);
const warningCount = computed(
  () => pageFindings.value.filter((f) => f.severity === "warning").length,
);

function applyAudit(audit: PageAudit) {
  pageMeta.value = audit.meta;
  pageFindings.value = audit.findings;
  pageResources.value = audit.resources;
}

/** Ask the bridge for the page's HTML; the audit runs here when it arrives. */
function refreshPageDetails() {
  postToFrame("snapshot");
}

/** The frame, plus the Page details sidebar beside it when that is open. */
const splitItems = computed<SplitterItem[]>(() => [
  {
    id: "preview-frame",
    slot: "frame",
    order: 1,
    defaultSize: detailsOpen.value ? 72 : 100,
    minSize: detailsOpen.value ? 40 : 100,
  },
  ...(detailsOpen.value
    ? [
        {
          id: "page-details",
          slot: "details",
          order: 2,
          defaultSize: 28,
          minSize: 16,
          maxSize: 45,
          collapsible: true,
          collapsedSize: 0,
        },
      ]
    : []),
]);

function onSplitCollapse(index: number) {
  if (splitItems.value[index]?.id === "page-details") detailsOpen.value = false;
}

function togglePageDetails() {
  detailsOpen.value = !detailsOpen.value;
  if (detailsOpen.value) refreshPageDetails();
}

// --- Page errors -------------------------------------------------------------------------------

/** The error to offer a fix for: an error overlay wins over uncaught errors. */
const pageError = ref<PageError | null>(null);

function onPageError(error: PageError) {
  if (error.kind === "build" || pageError.value?.kind !== "build") pageError.value = error;
}

function fixPageError() {
  if (!pageError.value) return;
  store.fixPageError(pageError.value, currentPath.value);
  pageError.value = null;
}

// --- RTL / dark mode -----------------------------------------------------------------------------

function syncAppearance() {
  postToFrame("appearance", { rtl: store.rtl, dark: store.dark });
}

watch(() => [store.rtl, store.dark], () => syncAppearance());

// --- Element picker ----------------------------------------------------------------------------

/** Ask the page to start or stop picking to match `store.picking`; the page owns the overlay. */
function syncPicker() {
  if (store.picking && state.value !== "ready") {
    store.setPicking(false);
    return;
  }
  postToFrame(store.picking ? "pick:start" : "pick:stop");
}

/**
 * Mention a picked element. The page sends its screenshot as a data URL; the daemon saves it
 * for the agent, and the mention carries the saved file's path instead.
 */
async function pickElement(picked: PickedElement) {
  const projectId = props.projectId;
  const { screenshot, ...rest } = picked;
  const mention: PickedElement = rest;
  if (mention.file) mention.file = relativeFile(mention.file, projectId);
  store.setPicking(false);
  const shot = screenshot ? dataUrlToBlob(screenshot) : null;
  if (shot) {
    try {
      mention.screenshot = await daemon.api.attachChatFile(projectId, shot);
    } catch (e) {
      // The mention is still useful without the picture.
      console.warn("element screenshot failed", e);
    }
  }
  store.pick(mention);
}

watch(() => store.picking, syncPicker);
// A remount (reload, project switch) replaces the document; `onLoad` asks the new one.
watch(() => [props.projectId, store.reloadNonce, store.navigateNonce, state.value], () => {
  if (state.value !== "ready") store.setPicking(false);
});
onBeforeUnmount(() => {
  store.setPicking(false);
});

/**
 * Step the frame back/forward. The frame's own history is hidden from the app (and shared with
 * the app's session history), so the app navigates it to the path it recorded instead.
 */
function step(direction: "back" | "forward") {
  const path = history.target(direction === "back" ? -1 : 1);
  if (!path) return;
  if (direction === "back") history.back(); else history.forward();
  startHref.value = loadHref(path);
}

function goBack() { step("back"); }
function goForward() { step("forward"); }

function navigate() {
  if (draft.value === null) return;
  const typed = pageHref(draft.value);
  draft.value = null;
  if (!typed) return;
  // The frame hides its path: remember the one we asked for, so a reload or a pick session
  // reopens the same page.
  const path = framePath(typed, "/");
  store.setFramePath(props.projectId, path);
  startHref.value = loadHref(path);
}

/**
 * The page on its preview origin, with the first-load token: a browser tab (or the default
 * browser the desktop shell hands loopback links to) has no cookie for that origin yet.
 */
function openExternal() {
  const href = loadHref(currentPath.value);
  if (href) window.open(href, "_blank", "noopener");
}

</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="preview-panel">
    <div data-testid="preview-shell" :class="shellClass">
      <div class="bg-background flex h-10 shrink-0 items-center gap-0.5 border-b border-default px-1.5">
        <UTooltip text="Back">
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            icon="i-hugeicons-arrow-left-01"
            aria-label="Back"
            :disabled="!history.canGoBack.value"
            @click="goBack"
          />
        </UTooltip>
        <UTooltip text="Forward">
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            icon="i-hugeicons-arrow-right-01"
            aria-label="Forward"
            :disabled="!history.canGoForward.value"
            @click="goForward"
          />
        </UTooltip>
        <UTooltip text="Reload">
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            icon="i-hugeicons-reload"
            aria-label="Reload preview"
            @click="store.reload()"
          />
        </UTooltip>

        <form class="mx-1 flex min-w-0 flex-1" @submit.prevent="navigate">
          <UInput
            :model-value="draft ?? currentPath"
            size="sm"
            class="min-w-0 flex-1"
            aria-label="Preview address"
            spellcheck="false"
            autocomplete="off"
            data-testid="preview-address"
            @update:model-value="draft = String($event)"
            @focus="($event.target as HTMLInputElement).select()"
            @blur="draft = null"
            @keydown.esc="draft = null; ($event.target as HTMLInputElement).blur()"
          >
            <template #leading>
              <UTooltip :text="LABEL[state]">
                <span
                  class="size-1.5 shrink-0 rounded-full"
                  :class="DOT[state]"
                  :data-state="state"
                  role="status"
                  :aria-label="LABEL[state]"
                  data-testid="status-dot"
                />
              </UTooltip>
            </template>
          </UInput>
        </form>

        <CleanupChangeButton :project-id="projectId" />

        <UDropdownMenu :items="deviceItems" :content="{ align: 'end', side: 'bottom' }">
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            :icon="device.icon"
            :square="!zoomed"
            :aria-label="`Viewport: ${device.label}${zoomed ? `, ${zoomPercent}%` : ''}`"
            data-testid="viewport-size"
          >
            <span v-if="zoomed" class="text-muted text-xs tabular-nums">{{ zoomPercent }}%</span>
          </UButton>
          <template #item-trailing="{ item }">
            <span class="text-muted pl-3 text-xs">{{ item.width }}</span>
          </template>
        </UDropdownMenu>
        <UDropdownMenu :items="appearanceItems" :content="{ align: 'end', side: 'bottom' }">
          <UButton
            color="neutral"
            :variant="store.rtl || store.dark ? 'soft' : 'ghost'"
            size="sm"
            :icon="store.dark ? 'i-hugeicons-moon-02' : 'i-hugeicons-sun-03'"
            aria-label="Appearance"
            data-testid="preview-appearance"
          />
          <template #item-label="{ item }">
            <span :data-testid="item.testid">{{ item.label }}</span>
          </template>
        </UDropdownMenu>
        <UTooltip :text="store.picking ? 'Cancel selection (Esc)' : 'Select an element to mention in the chat'">
          <UButton
            color="neutral"
            :variant="store.picking ? 'soft' : 'ghost'"
            size="sm"
            icon="i-hugeicons-cursor-pointer-01"
            aria-label="Select an element"
            :aria-pressed="store.picking"
            :disabled="state !== 'ready' || crossSite"
            data-testid="element-picker"
            @click="store.setPicking(!store.picking)"
          />
        </UTooltip>
        <UTooltip text="Page details">
          <UChip
            :show="errorCount + warningCount > 0"
            :color="errorCount > 0 ? 'error' : 'warning'"
            size="3xl"
            inset
            :ui="{ base: 'px-0.5 tabular-nums' }"
          >
            <UButton
              color="neutral"
              :variant="detailsOpen ? 'soft' : 'ghost'"
              size="sm"
              icon="i-hugeicons-file-search"
              aria-label="Page details"
              :aria-pressed="detailsOpen"
              :disabled="state !== 'ready' || crossSite"
              data-testid="page-details-toggle"
              @click="togglePageDetails"
            />
            <template #content>
              <span data-testid="page-details-badge">{{ errorCount + warningCount }}</span>
            </template>
          </UChip>
        </UTooltip>
        <UTooltip text="Open in new tab">
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            icon="i-hugeicons-link-square-02"
            aria-label="Open in new tab"
            @click="openExternal"
          />
        </UTooltip>
      </div>

      <USplitter
        :items="splitItems"
        auto-save-id="preview-page-details"
        class="relative min-h-0 flex-1"
        :ui="{ handle: 'border-default flex items-center justify-center border-l' }"
        @collapse="onSplitCollapse"
      >
        <template #resize-handle>
          <div
            class="bg-accented group-hover:bg-inverted/70 group-data-[resize-handle-state=drag]:bg-inverted h-8 w-1 rounded-full transition-colors"
            aria-hidden="true"
          />
        </template>
        <template #frame>
          <div class="relative flex h-full min-h-0 min-w-0 flex-col">
            <div ref="viewport" class="bg-background flex min-h-0 flex-1 justify-center overflow-clip">
              <UEmpty
                v-if="state === 'crashed'"
                variant="naked"
                icon="i-hugeicons-server-stack-03"
                :title="`The dev server crashed${store.message[projectId] ? `: ${store.message[projectId]}` : '.'}`"
                :description="store.error ?? undefined"
                :ui="{ description: 'text-error' }"
                role="alert"
              >
                <template #actions>
                  <UButton
                    :loading="store.restarting"
                    :label="store.restarting ? 'Restarting…' : 'Restart dev server'"
                    @click="store.restart(projectId)"
                  />
                  <UButton
                    color="neutral"
                    variant="outline"
                    icon="i-hugeicons-sparkles"
                    label="Fix this with your assistant"
                    data-testid="fix-crash"
                    @click="store.fixCrash(projectId)"
                  />
                </template>
              </UEmpty>
              <UEmpty
                v-else-if="state === 'starting'"
                variant="naked"
                title="Starting dev server…"
                data-testid="preview-starting"
              >
                <template #leading>
                  <UIcon
                    name="i-hugeicons-loading-03"
                    class="text-muted size-5 animate-spin"
                    role="status"
                    aria-label="Loading"
                  />
                </template>
              </UEmpty>
              <UEmpty
                v-else-if="crossSite"
                variant="naked"
                icon="i-hugeicons-shield-alert"
                title="Live preview isn't available from this page"
                :description="`The daemon at ${daemonOrigin} is on a different site than this app, so the browser won't keep the preview's sign-in cookie in this frame. Open the daemon link directly (the one printed at startup) to use the live preview.`"
                class="max-w-md"
                role="note"
                data-testid="preview-cross-site"
              >
                <template #actions>
                  <UButton color="neutral" variant="outline" size="sm" label="Try anyway" @click="forceFrame = true" />
                </template>
              </UEmpty>
              <UEmpty
                v-else-if="!startHref"
                variant="naked"
                :icon="accessError ? 'i-hugeicons-alert-02' : undefined"
                :title="accessError ? 'The preview could not be opened.' : 'Opening the preview…'"
                :description="accessError ?? undefined"
                :ui="{ description: 'text-error' }"
                data-testid="preview-opening"
              >
                <template v-if="accessError" #actions>
                  <UButton color="neutral" variant="outline" size="sm" label="Try again" @click="loadAccess()" />
                </template>
              </UEmpty>
              <!-- Sized to the scaled frame, so the panel centres and clips what is actually shown.
                   `overflow-clip` (here and on the viewport), not hidden/auto: a focus or
                   scrollIntoView inside the framed page also scrolls the parent's scroll
                   containers, and WebKit then shifts the scaled frame off to one side. -->
              <div
                v-else
                class="relative h-full shrink-0 overflow-clip"
                data-testid="preview-canvas"
                :style="zoomed ? { width: `${fit.width! * fit.scale}px` } : { width: fit.width ? `${fit.width}px` : '100%' }"
              >
                <!-- The frame backdrop stands in for the framed site's own page background in the
                     chosen preview scheme, not the app theme, so it keeps fixed colours on purpose. -->
                <iframe
                  ref="frame"
                  :key="`${projectId}:${store.reloadNonce}:${store.navigateNonce}`"
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
                <!-- A device-sized frame has no visible edge against the panel; outline it. -->
                <div
                  v-if="store.device !== 'desktop'"
                  class="border-default pointer-events-none absolute inset-0 border"
                  data-testid="preview-boundary"
                  aria-hidden="true"
                />
              </div>
            </div>

            <UAlert
              v-if="pageError && state === 'ready'"
              color="error"
              variant="outline"
              orientation="horizontal"
              icon="i-hugeicons-alert-02"
              :title="pageError.kind === 'build' ? 'This page failed to build' : 'This page threw an error'"
              :description="pageError.message"
              class="bg-default absolute inset-x-3 bottom-3 w-auto shadow-lg"
              :ui="{ description: 'text-muted line-clamp-2 font-mono text-xs break-words' }"
              role="alert"
              data-testid="page-error"
            >
              <template #actions>
                <UButton
                  size="sm"
                  icon="i-hugeicons-sparkles"
                  label="Fix this with your assistant"
                  data-testid="fix-page-error"
                  @click="fixPageError"
                />
                <UButton
                  color="neutral"
                  variant="ghost"
                  size="sm"
                  icon="i-hugeicons-cancel-01"
                  aria-label="Dismiss error"
                  @click="pageError = null"
                />
              </template>
            </UAlert>
          </div>
        </template>
        <template #details>
          <PageDetailsSidebar
            :meta="pageMeta"
            :findings="pageFindings"
            :resources="pageResources"
            :path="currentPath"
            :page-url="pageHref(currentPath)"
            @close="detailsOpen = false"
            @refresh="refreshPageDetails"
            @fix="store.fixCheck($event)"
          />
        </template>
      </USplitter>
    </div>
  </div>
</template>
