<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import type { UIMessage } from "ai";
import UChatMessages from "@nuxt/ui/components/ChatMessages.vue";

/**
 * The chat transcript: Nuxt UI's `UChatMessages` inside the panel's scroll container.
 *
 * `UChatMessages` follows the bottom while `status` is `streaming` (unless the user scrolled up)
 * and shows its "Scroll to bottom" button whenever the viewport is away from the end. This
 * wrapper adds what it does not do on its own:
 * - loading earlier history when the user scrolls near the top (`loadEarlier`), keeping the
 *   visible messages in place when they are prepended;
 * - keeping up with the end outside streaming (the checkpoint row that lands after a turn,
 *   a reloaded transcript) when the viewport was already at the bottom;
 * - jumping to the end when the user sends a message.
 */
const props = defineProps<{
  messages: UIMessage[];
  /** A turn is running: follow its output. */
  streaming: boolean;
  /** Older messages exist above the first rendered one. */
  canLoadEarlier: boolean;
  /** Classes for the message list (inside the scroll container). */
  listClass?: unknown;
}>();
const emit = defineEmits<{ loadEarlier: [] }>();
defineSlots<{ default(props: { message: UIMessage }): unknown }>();

/** Same distance `UChatMessages` treats as "at the bottom". */
const BOTTOM_THRESHOLD = 100;
/** Scrolling this close to the top asks for older messages. */
const LOAD_EARLIER_AT = 80;

const viewport = ref<HTMLElement | null>(null);

watch(
  () => props.messages,
  (next, prev) => {
    const el = viewport.value;
    if (!el) return;
    const fromBottom = el.scrollHeight - el.scrollTop;
    const atBottom = fromBottom - el.clientHeight < BOTTOM_THRESHOLD;
    const prevFirst = prev?.[0]?.id;
    const prepended = !!prevFirst && next[0]?.id !== prevFirst && next.some((m) => m.id === prevFirst);
    const last = next.at(-1);
    const sent = !!last && last.role === "user" && !prev?.some((m) => m.id === last.id);
    void nextTick(() => {
      // Earlier history went in above: keep the same messages under the user's eyes.
      if (prepended) el.scrollTop = el.scrollHeight - fromBottom;
      else if (sent || atBottom) el.scrollTop = el.scrollHeight;
    });
  },
  { flush: "pre" },
);

function onScroll(event: Event) {
  const el = event.target as HTMLElement;
  if (el.scrollTop <= LOAD_EARLIER_AT && props.canLoadEarlier) emit("loadEarlier");
}
</script>

<template>
  <div
    ref="viewport"
    class="scroll-fade-y relative min-h-0 flex-1 overflow-y-auto"
    data-testid="chat-viewport"
    @scroll="onScroll"
  >
    <UChatMessages
      :messages="messages"
      :status="streaming ? 'streaming' : 'ready'"
      should-auto-scroll
      compact
      data-testid="chat-messages"
      :class="listClass"
      :ui="{
        // The jump button sticks to the bottom edge of the scroll container.
        viewport: 'pointer-events-none sticky bottom-0 z-10 h-0',
        autoScroll: 'pointer-events-auto bottom-2',
      }"
    >
      <template #default>
        <template v-for="m in messages" :key="m.id">
          <slot :message="m" />
        </template>
      </template>
    </UChatMessages>
  </div>
</template>
