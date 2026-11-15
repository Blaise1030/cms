<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { ChatSummary } from "@/core/types";
import UButton from "@nuxt/ui/components/Button.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import UPopover from "@nuxt/ui/components/Popover.vue";

const props = defineProps<{ chats: ChatSummary[]; activeId: string | null; runningIds: string[] }>();
const emit = defineEmits<{ pick: [id: string]; remove: [id: string] }>();
const open = defineModel<boolean>("open", { default: false });
const pendingDelete = ref<ChatSummary | null>(null);
const query = ref("");

watch(open, (isOpen) => {
  if (!isOpen) query.value = "";
});

const when = (ms: number) => new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
const titleOf = (c: ChatSummary) => c.title || "Untitled chat";
/** A sync chat still combining its draft with the latest site. */
const combining = (c: ChatSummary) => c.kind === "sync" && c.sync?.phase === "working";

type ChatStatus = "running" | "combining" | "idle";

function statusOf(c: ChatSummary): ChatStatus {
  if (props.runningIds.includes(c.id) || c.running) return "running";
  if (combining(c)) return "combining";
  return "idle";
}

const statusLabel: Record<ChatStatus, string> = {
  running: "Running",
  combining: "Combining",
  idle: "Idle",
};

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return props.chats;
  return props.chats.filter((c) => titleOf(c).toLowerCase().includes(q));
});

function pick(id: string) {
  open.value = false;
  emit("pick", id);
}

function confirmDelete() {
  if (pendingDelete.value) emit("remove", pendingDelete.value.id);
  pendingDelete.value = null;
}
</script>

<template>
  <UPopover v-model:open="open" :content="{ align: 'start' }" :ui="{ content: 'flex w-72 flex-col' }">
    <UButton
      color="neutral"
      :variant="open ? 'soft' : 'ghost'"
      size="sm"
      icon="i-hugeicons-clock-04"
      aria-label="Chat history"
      data-testid="chat-history-toggle"
    />
    <template #content>
      <div class="border-default border-b p-2">
        <UInput
          v-model="query"
          icon="i-hugeicons-search-01"
          size="sm"
          placeholder="Search chats…"
          aria-label="Search chats"
          class="w-full"
          data-testid="chat-history-search"
          @keydown.stop
        />
      </div>
      <nav
        aria-label="Chat history"
        data-testid="chat-history"
        class="flex max-h-72 flex-col gap-1 overflow-y-auto p-1.5"
      >
        <p v-if="chats.length === 0" class="text-muted p-2 text-sm">No chats yet</p>
        <p v-else-if="filtered.length === 0" class="text-muted p-2 text-sm">No matching chats</p>
        <div
          v-for="c in filtered"
          :key="c.id"
          class="group relative flex items-center rounded-md transition-colors"
          :class="c.id === activeId ? 'bg-primary/10 hover:bg-primary/15' : 'hover:bg-elevated'"
        >
          <UButton
            color="neutral"
            variant="link"
            class="text-default hover:text-default flex min-w-0 flex-1 items-start gap-2 px-2 py-1.5 text-left text-sm font-normal"
            :aria-current="c.id === activeId ? 'true' : undefined"
            :aria-label="`${titleOf(c)}, ${statusLabel[statusOf(c)]}`"
            data-testid="chat-history-item"
            :data-status="statusOf(c)"
            @click="pick(c.id)"
          >
            <span class="mt-0.5 shrink-0" :title="statusLabel[statusOf(c)]" data-testid="chat-history-status">
              <UIcon
                v-if="statusOf(c) === 'running'"
                name="i-hugeicons-loading-03"
                class="size-3.5 animate-spin"
                aria-hidden="true"
              />
              <UIcon
                v-else-if="statusOf(c) === 'combining'"
                name="i-hugeicons-refresh"
                class="text-muted size-3.5 animate-spin"
                aria-hidden="true"
              />
              <UIcon v-else name="i-hugeicons-message-01" class="text-muted size-3.5 opacity-70" aria-hidden="true" />
            </span>
            <span class="min-w-0 flex-1">
              <span class="block w-full truncate">{{ titleOf(c) }}</span>
              <span class="text-muted block truncate text-xs">{{ when(c.updatedAt) }}</span>
            </span>
          </UButton>
          <UButton
            v-if="!runningIds.includes(c.id)"
            color="neutral"
            variant="ghost"
            size="sm"
            icon="i-hugeicons-delete-02"
            class="text-muted hover:text-error mr-1 shrink-0 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
            :aria-label="`Delete ${titleOf(c)}`"
            data-testid="chat-history-delete"
            @click="pendingDelete = c"
          />
        </div>
      </nav>
    </template>
  </UPopover>

  <UModal
    :open="pendingDelete !== null"
    title="Delete this chat?"
    :description="`“${pendingDelete ? titleOf(pendingDelete) : ''}” and its messages will be removed. Changes your assistant made to your site stay.`"
    :close="false"
    @update:open="(o) => !o && (pendingDelete = null)"
  >
    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton color="neutral" variant="outline" label="Cancel" @click="pendingDelete = null" />
        <UButton color="error" label="Delete chat" data-testid="chat-delete-confirm" @click="confirmDelete" />
      </div>
    </template>
  </UModal>
</template>
