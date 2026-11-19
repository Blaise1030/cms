<script setup lang="ts">
import { computed, ref } from "vue";
import type { DropdownMenuItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import { DESKTOP_MIN_WIDTH } from "@/modules/preview/frameFit";
import { DEVICE_WIDTH, type Device } from "@/modules/preview/store";
import BrowserFramePane from "./BrowserFramePane.vue";
import { frameUrl, normalizePath } from "./frameUrl";

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
const props = defineProps<{ frames: BrowserFrameSide[] }>();
defineSlots<{ placeholder(props: { frame: BrowserFrameSide }): unknown }>();
const device = defineModel<Device>("device", { default: "desktop" });
/**
 * With several sides, the key of the one folded to a narrow strip (at most one: folding a side
 * unfolds the other), so the rest fill the width.
 */
const collapsed = defineModel<string | null>("collapsed", { default: null });
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
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="browser-frame">
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
        <BrowserFramePane
          :src="srcs[i] ?? null"
          :title="frame.label"
          :device="device"
          :reload-nonce="reloadNonce"
          :testid="`browser-frame-iframe-${frame.key}`"
          @fit="scales = { ...scales, [frame.key]: $event }"
        >
          <slot name="placeholder" :frame="frame" />
        </BrowserFramePane>
      </section>
      </template>
    </div>
  </div>
</template>
