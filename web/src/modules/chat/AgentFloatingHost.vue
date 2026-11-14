<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Plus, X } from "@lucide/vue";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import {
  applySessionOrder,
  moveSessionId,
  readSessionOrder,
  writeSessionOrder,
} from "@/lib/sessionOrder";
import { cn } from "@/lib/utils";
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

/** Every available session, plus a draft tab when the active id is not in the list yet. */
const tabs = computed(() =>
  tabIds.value.map((id) => {
    const summary = chats.value.find((c) => c.id === id);
    return {
      id,
      title: summary?.title?.trim() || "New chat",
      running: store.isChatRunning(props.projectId, id) || !!summary?.running,
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

function onDragStart(event: DragEvent, id: string) {
  dragId.value = id;
  event.dataTransfer?.setData("text/plain", id);
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
}

function onDragOver(event: DragEvent) {
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
}

function onDrop(event: DragEvent, toId: string) {
  event.preventDefault();
  const fromId = dragId.value ?? event.dataTransfer?.getData("text/plain");
  dragId.value = null;
  if (!fromId) return;
  persistOrder(moveSessionId(tabIds.value, fromId, toId));
}

function onDragEnd() {
  dragId.value = null;
}

defineExpose({ expand, minimize, close: closePanel });
</script>

<template>
  <div data-testid="agent-floating-host" class="pointer-events-none fixed inset-0 z-40">
    <div
      v-if="mounted"
      v-show="expanded"
      data-testid="agent-floating-panel"
      class="pointer-events-auto absolute right-4 bottom-14 flex h-[min(40rem,calc(100vh-5rem))] w-[24rem] flex-col overflow-hidden rounded-xl border border-border bg-background shadow-lg"
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
        <button
          v-for="tab in tabs"
          :key="tab.id"
          type="button"
          role="tab"
          draggable="true"
          data-testid="agent-session-tab"
          :aria-selected="tab.active"
          :aria-current="tab.active ? 'page' : undefined"
          :aria-label="tab.running ? `${tab.title}, working` : tab.title"
          :class="
            cn(
              'group relative flex h-7 max-w-48 shrink-0 items-center gap-1.5 rounded-full py-0 pl-3 pr-3 text-left text-xs transition-all active:translate-y-px',
              // Close control is absolutely positioned so it does not reserve width when hidden.
              !tab.running && 'hover:pr-6 focus-within:pr-6',
              tab.active
                ? 'bg-primary/12 text-primary btn-raised-soft hover:bg-primary/16 hover:text-primary dark:bg-primary/25 dark:text-primary-foreground dark:hover:bg-primary/30'
                : 'text-muted-foreground hover:bg-background hover:text-foreground hover:btn-raised-soft dark:hover:bg-accent/50',
              dragId === tab.id && 'opacity-60',
            )
          "
          @click="openTab(tab.id)"
          @dragstart="onDragStart($event, tab.id)"
          @dragover="onDragOver"
          @drop="onDrop($event, tab.id)"
          @dragend="onDragEnd"
        >
          <Spinner
            v-if="tab.running"
            data-testid="agent-tab-status"
            data-state="running"
            class="size-3 shrink-0"
            aria-hidden="true"
          />
          <span
            v-else
            data-testid="agent-tab-status"
            data-state="idle"
            class="bg-muted-foreground/40 size-1.5 shrink-0 rounded-full"
            aria-hidden="true"
          />
          <span class="min-w-0 flex-1 truncate">{{ tab.title }}</span>
          <Button
            v-if="!tab.running"
            type="button"
            variant="ghost"
            size="icon-xs"
            class="text-muted-foreground hover:text-destructive absolute top-1/2 right-0.5 size-5 -translate-y-1/2 rounded-full p-0 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100 focus-visible:opacity-100"
            data-testid="agent-tab-delete"
            :aria-label="`Delete ${tab.title}`"
            @click="requestDelete($event, tab)"
          >
            <X class="size-3" />
          </Button>
        </button>
      </div>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class="m-1 shrink-0 self-center rounded-full"
        data-testid="agent-new-tab"
        aria-label="New agent session"
        @click="newTab"
      >
        <Plus />
      </Button>
    </div>

    <AlertDialog :open="pendingDelete !== null" @update:open="(o) => !o && (pendingDelete = null)">
      <AlertDialogContent class="pointer-events-auto">
        <AlertDialogHeader>
          <AlertDialogTitle>Delete this chat?</AlertDialogTitle>
          <AlertDialogDescription>
            “{{ pendingDelete?.title }}” and its messages will be removed. Changes your assistant
            made to your site stay.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <Button variant="destructive" data-testid="agent-tab-delete-confirm" @click="confirmDelete">
            Delete chat
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  </div>
</template>
