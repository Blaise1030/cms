<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import {
  applySessionOrder,
  moveSessionId,
  moveSessionToIndex,
  readSessionOrder,
  writeSessionOrder,
} from "@/lib/sessionOrder";
import { cn } from "@/lib/utils";
import { titleParts } from "./references";
import AgentPanel from "./AgentPanel.vue";
import { useChatStore } from "./store";

const props = defineProps<{ projectId: string }>();

const store = useChatStore();

/** Expanded floating card vs collapsed to the session bar. */
const expanded = ref(false);
/** Keep the panel mounted after the first open so in-flight turns survive minimize. */
const mounted = ref(false);
/** Local drag-reorder preference; unknown chats append at the end. */
const order = ref<string[]>(readSessionOrder(props.projectId));
const pendingDelete = ref<{ id: string; title: string } | null>(null);
const dragId = ref<string | null>(null);

const chats = computed(() => store.chats[props.projectId] ?? []);
const activeId = computed(() => store.activeChatId(props.projectId));

const tabIds = computed(() => {
  const ids = chats.value.map((c) => c.id);
  const active = activeId.value;
  if (active && !ids.includes(active)) ids.push(active);
  return applySessionOrder(ids, order.value);
});

/**
 * What a session is doing, from what the chat store knows: a turn in flight, a sync chat
 * combining, a queue stopped until the user resumes it, or nothing.
 */
type TabStatus = "running" | "combining" | "paused" | "idle";

const STATUS: Record<TabStatus, { label: string; icon?: string; class: string }> = {
  running: { label: "Working…", icon: "i-hugeicons-loading-03", class: "text-primary animate-spin" },
  combining: { label: "Combining…", icon: "i-hugeicons-refresh", class: "text-primary animate-spin" },
  paused: { label: "Queue paused, needs you", icon: "i-hugeicons-pause-circle", class: "text-warning" },
  idle: { label: "Idle", class: "bg-accented" },
};

function statusOf(id: string): TabStatus {
  const summary = chats.value.find((c) => c.id === id);
  if (store.isChatRunning(props.projectId, id) || summary?.running) return "running";
  if (summary?.kind === "sync" && (summary.sync?.phase === "working" || summary.sync?.phase === "checking"))
    return "combining";
  if (store.isQueuePaused(props.projectId, id) && store.queued[props.projectId]?.[id]?.length) return "paused";
  return "idle";
}

/** Every available session, plus a draft tab when the active id is not in the list yet. */
const tabs = computed(() =>
  tabIds.value.map((id) => {
    const summary = chats.value.find((c) => c.id === id);
    const status = statusOf(id);
    const title = summary?.title?.trim() || "New chat";
    return {
      id,
      title,
      parts: titleParts(title),
      status,
      running: status === "running",
      active: activeId.value === id,
    };
  }),
);

onMounted(() => {
  void store.load(props.projectId);
});

function persistOrder(ids: string[]) {
  order.value = ids;
  writeSessionOrder(props.projectId, ids);
}

function expand() {
  let id = store.activeChatId(props.projectId);
  if (!id) id = store.newChat(props.projectId);
  store.setActive(props.projectId, id);
  mounted.value = true;
  expanded.value = true;
}

function openTab(id: string) {
  store.setActive(props.projectId, id);
  mounted.value = true;
  expanded.value = true;
}

function newTab() {
  const id = store.newChat(props.projectId);
  persistOrder([...tabIds.value.filter((tabId) => tabId !== id), id]);
  mounted.value = true;
  expanded.value = true;
}

function minimize() {
  expanded.value = false;
}

function closePanel() {
  expanded.value = false;
}

function requestDelete(event: Event, tab: { id: string; title: string }) {
  event.stopPropagation();
  pendingDelete.value = tab;
}

async function confirmDelete() {
  const target = pendingDelete.value;
  pendingDelete.value = null;
  if (!target) return;
  const saved = chats.value.some((c) => c.id === target.id);
  if (saved) {
    try {
      await store.remove(props.projectId, target.id);
    } catch {
      // Daemon refuses (e.g. still running).
      return;
    }
  } else if (activeId.value === target.id) {
    // Unsaved draft — just open a fresh session.
    store.newChat(props.projectId);
  }
  persistOrder(tabIds.value.filter((id) => id !== target.id));
  if (tabIds.value.filter((id) => id !== target.id).length === 0) expanded.value = false;
}

/** The order before a drag, restored when the drag ends anywhere but on a tab. */
let beforeDrag: string[] = [];
let dropped = false;

function onDragStart(event: DragEvent, id: string) {
  dragId.value = id;
  beforeDrag = tabIds.value;
  dropped = false;
  event.dataTransfer?.setData("text/plain", id);
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
}

/** Reorder live as the dragged tab passes over another, so the landing spot is always visible. */
function onDragOver(event: DragEvent, overId: string) {
  if (!dragId.value) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  if (overId !== dragId.value) order.value = moveSessionId(tabIds.value, dragId.value, overId);
}

function onDrop(event: DragEvent) {
  event.preventDefault();
  dropped = true;
  persistOrder(tabIds.value);
}

function onDragEnd() {
  if (dragId.value && !dropped) order.value = beforeDrag;
  dragId.value = null;
}

/** Alt+Left / Alt+Right move the focused tab, for keyboard users. */
function onTabKeydown(event: KeyboardEvent, id: string) {
  if (!event.altKey || (event.key !== "ArrowLeft" && event.key !== "ArrowRight")) return;
  event.preventDefault();
  const ids = tabIds.value;
  const to = ids.indexOf(id) + (event.key === "ArrowLeft" ? -1 : 1);
  if (to < 0 || to >= ids.length) return;
  persistOrder(moveSessionToIndex(ids, id, to));
  requestAnimationFrame(() =>
    (document.querySelector(`[data-testid="agent-session-tab"][data-id="${CSS.escape(id)}"]`) as HTMLElement | null)?.focus(),
  );
}

defineExpose({ expand, minimize, close: closePanel });
</script>

<template>
  <div data-testid="agent-floating-host" class="pointer-events-none fixed inset-0 z-40">
    <div
      v-if="mounted"
      v-show="expanded"
      data-testid="agent-floating-panel"
      class="border-default bg-background pointer-events-auto absolute right-4 bottom-14 flex h-[min(40rem,calc(100vh-5rem))] w-[24rem] flex-col overflow-hidden rounded-xl border shadow-lg"
    >
      <AgentPanel
        class="min-h-0 flex-1"
        :project-id="projectId"
        layout="floating"
        @minimize="minimize"
        @close="closePanel"
      />
    </div>

    <div
      class="pointer-events-auto absolute right-0 bottom-0 left-14 flex h-11 items-stretch bg-background"
      data-testid="agent-session-bar"
      role="tablist"
      aria-label="Agent sessions"
    >
      <div class="flex min-w-0 flex-1 items-center justify-end gap-1.5 overflow-x-auto px-2">
        <UButton
          v-for="tab in tabs"
          :key="tab.id"
          color="neutral"
          :variant="tab.active ? 'outline' : 'ghost'"
          role="tab"
          draggable="true"
          data-testid="agent-session-tab"
          :data-id="tab.id"
          :data-status="tab.status"
          :aria-selected="tab.active"
          :aria-current="tab.active ? 'page' : undefined"
          :aria-label="`${tab.title}, ${STATUS[tab.status].label}`"
          :title="`${tab.title}: ${STATUS[tab.status].label}. Drag, or Alt+←/→, to reorder`"
          :class="
            cn(
              'group relative flex h-7 max-w-48 shrink-0 items-center gap-1.5 rounded-full py-0 pl-3 pr-3 text-left text-xs font-normal transition-all active:translate-y-px',
              // Close control is absolutely positioned so it does not reserve width when hidden.
              !tab.running && 'hover:pr-6 focus-within:pr-6',
              tab.active
                ? 'bg-elevated text-highlighted font-medium'
                : 'text-muted hover:bg-elevated hover:text-highlighted',
              dragId === tab.id && 'opacity-60',
            )
          "
          @click="openTab(tab.id)"
          @dragstart="onDragStart($event, tab.id)"
          @dragover="onDragOver($event, tab.id)"
          @drop="onDrop"
          @dragend="onDragEnd"
          @keydown="onTabKeydown($event, tab.id)"
        >
          <UIcon
            v-if="STATUS[tab.status].icon"
            :name="STATUS[tab.status].icon!"
            data-testid="agent-tab-status"
            :data-state="tab.status"
            :class="cn('size-3.5 shrink-0', STATUS[tab.status].class)"
            aria-hidden="true"
          />
          <span
            v-else
            data-testid="agent-tab-status"
            :data-state="tab.status"
            :class="cn('size-1.5 shrink-0 rounded-full', STATUS[tab.status].class)"
            aria-hidden="true"
          />
          <!-- A picked element shows as its tag (div.page), like in the chat, not raw markup. -->
          <span
            v-for="(el, k) in tab.parts.elements.slice(0, 1)"
            :key="k"
            class="border-default bg-default/60 inline-flex shrink-0 items-center gap-1 rounded-sm border px-1 font-mono text-[11px]"
            data-testid="agent-tab-element"
          >
            <UIcon name="i-hugeicons-cursor-pointer-01" class="size-3" aria-hidden="true" />
            {{ el }}
          </span>
          <span v-if="tab.parts.text || !tab.parts.elements.length" class="min-w-0 flex-1 truncate">{{
            tab.parts.text || tab.title
          }}</span>
          <UButton
            v-if="!tab.running"
            type="button"
            color="neutral"
            variant="ghost"
            size="xs"
            icon="i-hugeicons-cancel-01"
            class="text-muted hover:text-error absolute top-1/2 right-0.5 size-5 -translate-y-1/2 justify-center rounded-full p-0 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100 focus-visible:opacity-100"
            :ui="{ leadingIcon: 'size-3' }"
            data-testid="agent-tab-delete"
            :aria-label="`Delete ${tab.title}`"
            @click="requestDelete($event, tab)"
          />
        </UButton>
      </div>
      <UButton
        type="button"
        color="neutral"
        variant="ghost"
        size="sm"
        icon="i-hugeicons-plus-sign"
        class="m-1 shrink-0 self-center rounded-full"
        data-testid="agent-new-tab"
        aria-label="New agent session"
        @click="newTab"
      />
    </div>

    <UModal
      :open="pendingDelete !== null"
      title="Delete this chat?"
      :description="`“${pendingDelete?.title ?? ''}” and its messages will be removed. Changes your assistant made to your site stay.`"
      :close="false"
      :ui="{ content: 'pointer-events-auto' }"
      @update:open="(o) => !o && (pendingDelete = null)"
    >
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="outline" label="Cancel" @click="pendingDelete = null" />
          <UButton color="error" label="Delete chat" data-testid="agent-tab-delete-confirm" @click="confirmDelete" />
        </div>
      </template>
    </UModal>
  </div>
</template>
