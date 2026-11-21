<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import type { DropdownMenuItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import { DESKTOP_MIN_WIDTH } from "@/modules/preview/frameFit";
import { DEVICE_WIDTH, type Device } from "@/modules/preview/store";
import BrowserFramePane from "./BrowserFramePane.vue";
import { frameUrl, normalizePath } from "./frameUrl";

/** An area dragged out on one side, in CSS pixels of the page (`clientX`/`clientY` space). */
export interface BrowserFramePick {
  side: string;
  /** The path the frames were showing. */
  path: string;
  device: Device;
  rect: { x: number; y: number; width: number; height: number };
}

/** A drag smaller than this on either side is a click, not an area. */
const MIN_PICK = 8;

/** One framed site. `url` is its base; `null` (or a non-http URL) shows the `placeholder` slot. */
export interface BrowserFrameSide {
  key: string;
  label: string;
  url: string | null;
  /** A short state shown on the side's strip while it is collapsed (e.g. "Deploying…"). */
  status?: string;
}

/**
 * A browser-like frame for one or more sites side by side, styled like the preview page's
 * toolbar. The sites are on other origins, so the frame can't see where a page navigates: the
 * address bar holds the path it opened, and opening a path opens it on every side. The path
 * survives sides coming and going; give the frame a new `key` to start again at `/`.
 */
const props = defineProps<{
  frames: BrowserFrameSide[];
  /** Offer "Pick an area to comment on" on each side's header. */
  pickable?: boolean;
}>();
const emit = defineEmits<{ pick: [area: BrowserFramePick] }>();
defineSlots<{ placeholder(props: { frame: BrowserFrameSide }): unknown }>();
const device = defineModel<Device>("device", { default: "desktop" });
/**
 * With several sides, the key of the one folded to a narrow strip (at most one: folding a side
 * unfolds the other), so the rest fill the width.
 */
const collapsed = defineModel<string | null>("collapsed", { default: null });
/** The side being picked on (its frame dimmed, a crosshair to drag an area), or null. */
const picking = defineModel<string | null>("picking", { default: null });
const isCollapsed = (frame: BrowserFrameSide) => props.frames.length > 1 && collapsed.value === frame.key;
/** "This change" → "this change", for "Hide this change". */
const lower = (label: string) => label.charAt(0).toLowerCase() + label.slice(1);

/** Paths opened from the address bar, with the one shown at `at`. */
const history = ref<string[]>(["/"]);
const at = ref(0);
const path = computed(() => history.value[at.value] ?? "/");
const draft = ref<string | null>(null);
const reloadNonce = ref(0);

function open(input: string) {
  const next = normalizePath(input);
  draft.value = null;
  if (next === path.value) return;
  history.value = [...history.value.slice(0, at.value + 1), next];
  at.value = history.value.length - 1;
}
const canGoBack = computed(() => at.value > 0);
const canGoForward = computed(() => at.value < history.value.length - 1);
const goBack = () => canGoBack.value && at.value--;
const goForward = () => canGoForward.value && at.value++;
const reload = () => reloadNonce.value++;

const srcs = computed(() => props.frames.map((f) => frameUrl(f.url, path.value)));

/** Each side fits its own width; the toolbar shows the first side's zoom (side by side, they match). */
const scales = ref<Record<string, number>>({});
const shown = computed(() => props.frames.find((f) => !isCollapsed(f)) ?? props.frames[0]);
const scale = computed(() => scales.value[shown.value?.key ?? ""] ?? 1);
const zoomed = computed(() => scale.value < 1);
const zoomPercent = computed(() => Math.round(scale.value * 100));

const DEVICES: { id: Device; label: string; icon: string }[] = [
  { id: "desktop", label: "Desktop", icon: "i-hugeicons-computer" },
  { id: "tablet", label: "Tablet", icon: "i-hugeicons-tablet-01" },
  { id: "mobile", label: "Mobile", icon: "i-hugeicons-smart-phone-01" },
];
const current = computed(() => DEVICES.find((d) => d.id === device.value) ?? DEVICES[0]!);
const deviceItems = computed<DropdownMenuItem[]>(() =>
  DEVICES.map((d) => ({
    type: "checkbox",
    label: d.label,
    icon: d.icon,
    checked: device.value === d.id,
    width: DEVICE_WIDTH[d.id] ? `${DEVICE_WIDTH[d.id]}px` : `${DESKTOP_MIN_WIDTH}px+`,
    testid: `browser-frame-device-${d.id}`,
    onUpdateChecked: () => (device.value = d.id),
  })),
);

function openExternal(src: string | null) {
  if (src) window.open(src, "_blank", "noopener,noreferrer");
}

/** The drag in progress, in the overlay's own coordinates. */
const drag = ref<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
const dragBox = computed(() => {
  const d = drag.value;
  if (!d) return null;
  return { left: Math.min(d.x0, d.x1), top: Math.min(d.y0, d.y1), width: Math.abs(d.x1 - d.x0), height: Math.abs(d.y1 - d.y0) };
});
function onPickDown(e: PointerEvent) {
  const el = e.currentTarget as HTMLElement;
  const box = el.getBoundingClientRect();
  el.setPointerCapture?.(e.pointerId);
  drag.value = { x0: e.clientX - box.left, y0: e.clientY - box.top, x1: e.clientX - box.left, y1: e.clientY - box.top };
}
function onPickMove(e: PointerEvent) {
  if (!drag.value) return;
  const box = (e.currentTarget as HTMLElement).getBoundingClientRect();
  drag.value = { ...drag.value, x1: e.clientX - box.left, y1: e.clientY - box.top };
}
async function onPickUp(e: PointerEvent, side: string) {
  const box = (e.currentTarget as HTMLElement).getBoundingClientRect();
  const area = dragBox.value;
  drag.value = null;
  if (!area || area.width < MIN_PICK || area.height < MIN_PICK) return;
  // The overlay leaves first, so a screenshot of the page shows the page, not the dimming.
  picking.value = null;
  await nextTick();
  emit("pick", {
    side,
    path: path.value,
    device: device.value,
    rect: { x: box.left + area.left, y: box.top + area.top, width: area.width, height: area.height },
  });
}
function onPickKey(e: KeyboardEvent) {
  if (e.key === "Escape" && picking.value) {
    drag.value = null;
    picking.value = null;
  }
}
watch(picking, (side) => {
  if (side) window.addEventListener("keydown", onPickKey);
  else window.removeEventListener("keydown", onPickKey);
});
onBeforeUnmount(() => window.removeEventListener("keydown", onPickKey));

/**
 * orso's preview bridge in a framed page (pull request previews built with `ORSO_PREVIEW=1`):
 * a `hello` on each load, answered with `ready` and whether the page can capture itself.
 */
const BRIDGE = "orso:preview";
const root = ref<HTMLElement | null>(null);
const bridges = ref<Record<string, { capture: boolean }>>({});
const waiting = new Map<number, { resolve: (url: string) => void; reject: (e: Error) => void }>();
let nextCapture = 0;
const frameOf = (key: string) =>
  root.value?.querySelector<HTMLIFrameElement>(`iframe[data-testid='browser-frame-iframe-${key}']`) ?? null;
/**
 * The origin of the site `key` frames (its `url`). The bridge is only spoken to there, and only
 * believed from there: a frame that navigated elsewhere is a different site.
 */
function originOf(key: string): string | null {
  const url = props.frames.find((f) => f.key === key)?.url;
  try {
    return url ? new URL(url).origin : null;
  } catch {
    return null;
  }
}

function onFrameLoad(key: string, frame: HTMLIFrameElement) {
  const { [key]: _gone, ...rest } = bridges.value;
  bridges.value = rest;
  const origin = originOf(key);
  if (origin) frame.contentWindow?.postMessage({ type: BRIDGE, op: "hello" }, origin);
}
function onBridgeMessage(e: MessageEvent) {
  const data = e.data as { type?: string; op?: string; id?: number; capture?: boolean; ok?: boolean; dataUrl?: string; error?: string };
  if (!data || data.type !== BRIDGE) return;
  // Only the frames this component shows.
  const key = props.frames.find((f) => frameOf(f.key)?.contentWindow === e.source)?.key;
  if (!key || e.origin !== originOf(key)) return;
  if (data.op === "ready") bridges.value = { ...bridges.value, [key]: { capture: !!data.capture } };
  else if (data.op === "capture" && typeof data.id === "number") {
    const pending = waiting.get(data.id);
    waiting.delete(data.id);
    if (!pending) return;
    if (data.ok && typeof data.dataUrl === "string" && data.dataUrl.startsWith("data:image/")) pending.resolve(data.dataUrl);
    else pending.reject(new Error(data.error || "The preview couldn't capture itself."));
  }
}
window.addEventListener("message", onBridgeMessage);
onBeforeUnmount(() => {
  window.removeEventListener("message", onBridgeMessage);
  for (const p of waiting.values()) p.reject(new Error("The frame went away."));
  waiting.clear();
});

/** Whether `side`'s page can screenshot itself (its bridge said so). */
const canCapture = (side: string) => !!bridges.value[side]?.capture;

/**
 * A PNG of `rect` (CSS px of the page) cut out of `side`'s page by its bridge: the rect is
 * turned into the frame's own CSS px (allowing for its zoom). Rejects when the page can't.
 */
function captureSide(side: string, rect: BrowserFramePick["rect"], timeoutMs = 10_000): Promise<string> {
  const frame = frameOf(side);
  const origin = originOf(side);
  if (!frame?.contentWindow || !origin || !canCapture(side)) return Promise.reject(new Error("No preview bridge."));
  const box = frame.getBoundingClientRect();
  const k = (frame.offsetWidth || box.width) / (box.width || 1);
  const x0 = Math.max(rect.x, box.left);
  const y0 = Math.max(rect.y, box.top);
  const x1 = Math.min(rect.x + rect.width, box.right);
  const y1 = Math.min(rect.y + rect.height, box.bottom);
  if (x1 - x0 < 1 || y1 - y0 < 1) return Promise.reject(new Error("The area is outside the page."));
  const local = { x: (x0 - box.left) * k, y: (y0 - box.top) * k, width: (x1 - x0) * k, height: (y1 - y0) * k };
  const id = ++nextCapture;
  return new Promise<string>((resolve, reject) => {
    const timer = setTimeout(() => {
      waiting.delete(id);
      reject(new Error("The preview didn't answer."));
    }, timeoutMs);
    waiting.set(id, {
      resolve: (url) => (clearTimeout(timer), resolve(url)),
      reject: (e) => (clearTimeout(timer), reject(e)),
    });
    frame.contentWindow!.postMessage({ type: BRIDGE, op: "capture", id, rect: local }, origin);
  });
}

defineExpose({ open, canCapture, captureSide });
</script>

<template>
  <div ref="root" class="flex h-full min-h-0 flex-col" data-testid="browser-frame">
    <div class="bg-background border-default flex h-10 shrink-0 items-center gap-0.5 border-b px-1.5">
      <UTooltip text="Back">
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          icon="i-hugeicons-arrow-left-01"
          aria-label="Back"
          :disabled="!canGoBack"
          data-testid="browser-frame-back"
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
          :disabled="!canGoForward"
          data-testid="browser-frame-forward"
          @click="goForward"
        />
      </UTooltip>
      <UTooltip text="Reload">
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          icon="i-hugeicons-arrow-turn-forward"
          aria-label="Reload"
          data-testid="browser-frame-reload"
          @click="reload"
        />
      </UTooltip>

      <form class="mx-1 flex min-w-0 flex-1" data-testid="browser-frame-form" @submit.prevent="open(draft ?? path)">
        <UInput
          :model-value="draft ?? path"
          size="sm"
          class="min-w-0 flex-1"
          aria-label="Page address"
          spellcheck="false"
          autocomplete="off"
          data-testid="browser-frame-address"
          @update:model-value="draft = String($event)"
          @focus="($event.target as HTMLInputElement).select()"
          @blur="draft = null"
          @keydown.esc="draft = null; ($event.target as HTMLInputElement).blur()"
        />
      </form>

      <UDropdownMenu :items="deviceItems" :content="{ align: 'end', side: 'bottom' }">
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          :icon="current.icon"
          :square="!zoomed"
          :aria-label="`Viewport: ${current.label}${zoomed ? `, ${zoomPercent}%` : ''}`"
          data-testid="browser-frame-size"
        >
          <span v-if="zoomed" class="text-muted text-xs tabular-nums">{{ zoomPercent }}%</span>
        </UButton>
        <template #item-trailing="{ item }">
          <span class="text-muted pl-3 text-xs">{{ item.width }}</span>
        </template>
      </UDropdownMenu>
      <!-- With one side the toolbar opens it; side by side, each side's header has its own. -->
      <UTooltip v-if="frames.length === 1" text="Open in new tab">
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          icon="i-hugeicons-link-square-02"
          aria-label="Open in new tab"
          :disabled="!srcs[0]"
          data-testid="browser-frame-open"
          @click="openExternal(srcs[0] ?? null)"
        />
      </UTooltip>
    </div>

    <div class="divide-default flex min-h-0 flex-1 divide-x">
      <template v-for="(frame, i) in frames" :key="frame.key">
      <!-- A folded side: a narrow strip with its label, state and a way back. -->
      <div
        v-if="isCollapsed(frame)"
        class="bg-background flex w-9 shrink-0 flex-col items-center gap-2 py-1"
        :data-testid="`browser-frame-strip-${frame.key}`"
      >
        <UTooltip :text="`Show ${lower(frame.label)}`">
          <UButton
            color="neutral"
            variant="ghost"
            size="xs"
            icon="i-hugeicons-sidebar-left-01"
            :aria-label="`Show ${lower(frame.label)}`"
            :data-testid="`browser-frame-expand-${frame.key}`"
            @click="collapsed = null"
          />
        </UTooltip>
        <span class="text-muted text-xs font-medium [writing-mode:vertical-rl]">{{ frame.label }}</span>
        <span
          v-if="frame.status"
          class="text-dimmed text-xs [writing-mode:vertical-rl]"
          :data-testid="`browser-frame-status-${frame.key}`"
        >{{ frame.status }}</span>
      </div>
      <!-- Kept mounted while folded, so unfolding doesn't reload the site. -->
      <section
        v-show="!isCollapsed(frame)"
        class="flex min-h-0 min-w-0 flex-1 flex-col"
        :data-testid="`browser-frame-side-${frame.key}`"
      >
        <div
          v-if="frames.length > 1"
          class="bg-background border-default flex h-8 shrink-0 items-center gap-1 border-b pr-1 pl-3"
        >
          <span class="text-muted min-w-0 flex-1 truncate text-xs font-medium">{{ frame.label }}</span>
          <UTooltip v-if="pickable" :text="picking === frame.key ? 'Stop picking (Esc)' : 'Pick an area to comment on'">
            <UButton
              color="neutral"
              :variant="picking === frame.key ? 'soft' : 'ghost'"
              size="xs"
              icon="i-hugeicons-crop"
              :aria-label="`Pick an area of ${lower(frame.label)} to comment on`"
              :aria-pressed="picking === frame.key"
              :disabled="!srcs[i]"
              :data-testid="`browser-frame-pick-${frame.key}`"
              @click="picking = picking === frame.key ? null : frame.key"
            />
          </UTooltip>
          <UTooltip :text="`Hide ${lower(frame.label)}`">
            <UButton
              color="neutral"
              variant="ghost"
              size="xs"
              icon="i-hugeicons-sidebar-right-01"
              :aria-label="`Hide ${lower(frame.label)}`"
              :data-testid="`browser-frame-collapse-${frame.key}`"
              @click="collapsed = frame.key"
            />
          </UTooltip>
          <UTooltip text="Open in new tab">
            <UButton
              color="neutral"
              variant="ghost"
              size="xs"
              icon="i-hugeicons-link-square-02"
              :aria-label="`Open ${frame.label} in new tab`"
              :disabled="!srcs[i]"
              data-testid="browser-frame-open"
              @click="openExternal(srcs[i] ?? null)"
            />
          </UTooltip>
        </div>
        <div class="relative flex min-h-0 flex-1 flex-col">
          <BrowserFramePane
            :src="srcs[i] ?? null"
            :title="frame.label"
            :device="device"
            :reload-nonce="reloadNonce"
            :testid="`browser-frame-iframe-${frame.key}`"
            @fit="scales = { ...scales, [frame.key]: $event }"
            @load="onFrameLoad(frame.key, $event)"
          >
            <slot name="placeholder" :frame="frame" />
          </BrowserFramePane>
          <!-- Picking: the page is dimmed and a drag marks the area to comment on. -->
          <div
            v-if="picking === frame.key"
            class="absolute inset-0 z-10 cursor-crosshair touch-none bg-black/25 select-none"
            :data-testid="`browser-frame-pick-overlay-${frame.key}`"
            @pointerdown.prevent="onPickDown"
            @pointermove="onPickMove"
            @pointerup="onPickUp($event, frame.key)"
          >
            <div
              v-if="dragBox"
              class="border-primary bg-primary/10 pointer-events-none absolute border-2"
              :style="{ left: `${dragBox.left}px`, top: `${dragBox.top}px`, width: `${dragBox.width}px`, height: `${dragBox.height}px` }"
              data-testid="browser-frame-pick-box"
            />
          </div>
        </div>
      </section>
      </template>
    </div>
  </div>
</template>
