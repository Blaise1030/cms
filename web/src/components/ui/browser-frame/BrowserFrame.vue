<script setup lang="ts">
import { computed, ref } from "vue";
import { useElementSize } from "@vueuse/core";
import type { DropdownMenuItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
import { DESKTOP_MIN_WIDTH, frameFit } from "@/modules/preview/frameFit";
import { DEVICE_WIDTH, type Device } from "@/modules/preview/store";
import { frameUrl, normalizePath } from "./frameUrl";

/** One framed site. `url` is its base; `null` (or a non-http URL) shows the `placeholder` slot. */
export interface BrowserFrameSide {
  key: string;
  label: string;
  url: string | null;
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

/** Every side gets the same width, so one measurement sizes them all. */
const pane = ref<HTMLElement | null>(null);
const paneSize = useElementSize(pane);
const fit = computed(() => frameFit(device.value, paneSize.width.value, paneSize.height.value));
const zoomed = computed(() => fit.value.scale < 1);
const zoomPercent = computed(() => Math.round(fit.value.scale * 100));

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
      <section
        v-for="(frame, i) in frames"
        :key="frame.key"
        class="flex min-h-0 min-w-0 flex-1 flex-col"
        :data-testid="`browser-frame-side-${frame.key}`"
      >
        <div
          v-if="frames.length > 1"
          class="bg-background border-default flex h-8 shrink-0 items-center gap-1 border-b pr-1 pl-3"
        >
          <span class="text-muted min-w-0 flex-1 truncate text-xs font-medium">{{ frame.label }}</span>
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
        <div
          :ref="i === 0 ? (el) => (pane = el as HTMLElement | null) : undefined"
          class="bg-background flex min-h-0 flex-1 justify-center overflow-auto"
        >
          <slot v-if="!srcs[i]" name="placeholder" :frame="frame" />
          <!-- Sized to the scaled frame, so the pane centres and clips what is actually shown. -->
          <div
            v-else
            class="relative h-full shrink-0 overflow-hidden"
            :style="zoomed ? { width: `${fit.width! * fit.scale}px` } : { width: fit.width ? `${fit.width}px` : '100%' }"
          >
            <!-- Another site: its scripts run in its own origin, never orso's. -->
            <iframe
              :key="`${srcs[i]}:${reloadNonce}`"
              :src="srcs[i]!"
              :title="frame.label"
              class="h-full origin-top-left border-0 bg-white"
              :style="{
                width: fit.width ? `${fit.width}px` : '100%',
                ...(zoomed ? { height: `${fit.height}px`, transform: `scale(${fit.scale})` } : {}),
              }"
              sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
              referrerpolicy="no-referrer"
              :data-testid="`browser-frame-iframe-${frame.key}`"
            />
            <div
              v-if="device !== 'desktop'"
              class="border-default pointer-events-none absolute inset-0 border"
              aria-hidden="true"
            />
          </div>
        </div>
      </section>
    </div>
  </div>
</template>
