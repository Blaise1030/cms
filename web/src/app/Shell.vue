<script setup lang="ts">
import { computed, nextTick, ref, useSlots, watch } from "vue";
import type { SplitterItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import USplitter from "@nuxt/ui/components/Splitter.vue";
import { isMacDesktop } from "@/lib/desktop";

const props = withDefaults(
  defineProps<{
    showChat?: boolean;
    /** The chat floats over the page, so its column collapses (but stays mounted). */
    chatDetached?: boolean;
    hideTopBar?: boolean;
  }>(),
  { showChat: true, chatDetached: false },
);

// The traffic lights end ~80px from the window's left edge; the padding leaves a clear gap after them (the 56px rail covers most of it).
const slots = useSlots();
const macDesktop = isMacDesktop();
const topBarPadding = !macDesktop ? "px-3" : slots.rail ? "pr-3 pl-10 fullscreen:pl-3" : "pr-3 pl-24 fullscreen:pl-3";

// Panel ids and the auto-save id match the old resizable layout, so saved widths carry over.
const panels = computed<SplitterItem[]>(() => [
  {
    id: "workspace",
    slot: "workspace-panel",
    defaultSize: props.showChat ? 65 : 100,
    minSize: props.showChat ? 30 : 100,
    class: "min-w-0",
  },
  ...(props.showChat
    ? [{ id: "chat", slot: "chat-panel", defaultSize: 35, minSize: 22, collapsible: true, collapsedSize: 0, class: "min-w-0" }]
    : []),
]);

/** reka-ui's SplitterPanel instances, in `panels` order. */
interface PanelHandle {
  collapse: () => void;
  expand: () => void;
  getSize: () => number;
  resize: (size: number) => void;
}
const splitter = ref<{ panelsRef: PanelHandle[] } | null>(null);
const workspacePanel = computed(() => splitter.value?.panelsRef[0] ?? null);
const chatPanel = computed(() => (props.showChat ? splitter.value?.panelsRef[1] ?? null : null));
/** The workspace's split before a page without chat takes the full width. */
let workspaceSize = 65;
const chatOpen = ref(true);

// Pages without an agent drop the chat column; a saved 65/35 split would leave its space empty.
watch(
  () => props.showChat,
  (show, was) => {
    if (!workspacePanel.value) return;
    if (!show) {
      workspaceSize = workspacePanel.value.getSize();
      workspacePanel.value.resize(100);
    } else if (was === false) {
      void nextTick(() => workspacePanel.value?.resize(workspaceSize));
    }
  },
);
function onCollapse(index: number) {
  if (index === 1) chatOpen.value = false;
}
function onExpand(index: number) {
  if (index === 1) chatOpen.value = true;
}

function toggleChat() {
  if (chatOpen.value) chatPanel.value?.collapse();
  else chatPanel.value?.expand();
}

function openChat() {
  if (!props.chatDetached) chatPanel.value?.expand();
}
defineExpose({ openChat });

watch(
  () => props.chatDetached,
  (detached) => {
    if (detached) chatPanel.value?.collapse();
    else chatPanel.value?.expand();
  },
);
</script>

<template>
  <div class="flex h-full overflow-hidden">
    <slot name="rail" />
    <div class="flex min-w-0 flex-1 flex-col overflow-hidden">
      <USplitter
        ref="splitter"
        :items="panels"
        auto-save-id="orso-shell"
        class="min-h-0 flex-1"
        :ui="{
          handle: [
            'flex w-px items-center justify-center bg-transparent after:absolute after:inset-y-0 after:left-1/2 after:w-2 after:-translate-x-1/2',
            chatDetached ? 'hidden' : '',
          ].join(' '),
        }"
        @collapse="onCollapse"
        @expand="onExpand"
      >
        <template #workspace-panel>
          <div class="flex h-full min-h-0 w-full flex-col overflow-hidden">
            <header
              v-if="!hideTopBar"
              :class="['bg-background flex h-11 shrink-0 items-center gap-3', topBarPadding]"
              :data-tauri-drag-region="macDesktop ? 'deep' : undefined"
              data-testid="top-bar"
            >
              <div class="flex min-w-0 flex-1 items-center gap-3">
                <slot name="menubar" />
              </div>
              <div class="flex shrink-0 items-center gap-3">
                <slot name="actions" />
                <UButton
                  v-if="showChat && !chatDetached"
                  type="button"
                  color="neutral"
                  variant="ghost"
                  icon="i-hugeicons-chat-bot"
                  :aria-label="chatOpen ? 'Hide chat' : 'Show chat'"
                  :aria-pressed="chatOpen"
                  data-testid="chat-toggle"
                  @click="toggleChat"
                />
              </div>
            </header>
            <!-- macOS desktop with no top bar: an empty strip keeps the page clear of the traffic
                 lights, level with the rail, and is still where the window is dragged from. -->
            <div
              v-else-if="macDesktop"
              class="bg-background h-11 shrink-0 fullscreen:h-0"
              data-tauri-drag-region
              data-testid="top-bar-spacer"
            />
            <div class="min-h-0 flex-1 overflow-hidden">
              <slot name="workspace" />
            </div>
          </div>
        </template>
        <template #resize-handle>
          <div
            class="bg-accented group-hover:bg-inverted/70 group-data-[resize-handle-state=drag]:bg-inverted z-10 h-8 w-1 shrink-0 rounded-full transition-colors"
            aria-hidden="true"
          />
        </template>
        <template #chat-panel>
          <div class="bg-background h-full min-h-0 w-full overflow-hidden">
            <slot name="chat" />
          </div>
        </template>
      </USplitter>
    </div>
  </div>
</template>
