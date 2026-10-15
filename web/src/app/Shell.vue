<script setup lang="ts">
import { ref, useSlots, watch } from "vue";
import { BotMessageSquare } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { isMacDesktop } from "@/lib/desktop";

const props = withDefaults(
  defineProps<{
    showChat?: boolean;
    /** The chat floats over the page, so its column collapses (but stays mounted). */
    chatDetached?: boolean;
  }>(),
  { showChat: true, chatDetached: false },
);

// The traffic lights end ~80px from the window's left edge; the 56px project rail covers most of that.
const slots = useSlots();
const macDesktop = isMacDesktop();
const topBarPadding = !macDesktop ? "px-3" : slots.rail ? "pr-3 pl-7" : "pr-3 pl-20";

const chatPanel = ref<{ collapse: () => void; expand: () => void } | null>(null);
const chatOpen = ref(true);
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
      <ResizablePanelGroup
        direction="horizontal"
        auto-save-id="workbench-shell"
        class="min-h-0 flex-1"
      >
        <ResizablePanel
          id="workspace"
          :default-size="showChat ? 65 : 100"
          :min-size="showChat ? 30 : 100"
        >
          <div class="flex h-full min-h-0 flex-col overflow-hidden">
            <header
              :class="['bg-background flex h-11 shrink-0 items-center gap-3', topBarPadding]"
              :data-tauri-drag-region="macDesktop ? 'deep' : undefined"
              data-testid="top-bar"
            >
              <div class="flex min-w-0 flex-1 items-center gap-3">
                <slot name="menubar" />
              </div>
              <div class="flex shrink-0 items-center gap-3">
                <slot name="actions" />
                <Button
                  v-if="showChat && !chatDetached"
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  :aria-label="chatOpen ? 'Hide chat' : 'Show chat'"
                  :aria-pressed="chatOpen"
                  data-testid="chat-toggle"
                  @click="toggleChat"
                >
                  <BotMessageSquare />
                </Button>
              </div>
            </header>
            <div class="min-h-0 flex-1 overflow-hidden">
              <slot name="workspace" />
            </div>
          </div>
        </ResizablePanel>
        <template v-if="showChat">
          <ResizableHandle v-show="!chatDetached" with-handle class="bg-transparent" />
          <ResizablePanel
            id="chat"
            ref="chatPanel"
            :default-size="35"
            :min-size="22"
            collapsible
            :collapsed-size="0"
            @collapse="chatOpen = false"
            @expand="chatOpen = true"
          >
            <div class="bg-background h-full min-h-0 overflow-hidden border-l">
              <slot name="chat" />
            </div>
          </ResizablePanel>
        </template>
      </ResizablePanelGroup>
    </div>
  </div>
</template>
