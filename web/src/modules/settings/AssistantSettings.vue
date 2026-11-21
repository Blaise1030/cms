<script setup lang="ts">
// App settings → Assistant. The same rows and sign-in steps as the setup dialog,
// without a chat to connect to: a ready row simply stays ready. Rows can be dragged
// (or moved with Alt+Up/Down) into the user's order; new chats start with the first
// connected one, marked Default.
import { computed, onMounted, ref } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import type { ProviderSignInState } from "@/core/types";
import ProviderRow from "@/modules/assistant/ProviderRow.vue";
import SignInPanel from "@/modules/assistant/SignInPanel.vue";
import { useAssistantStore } from "@/modules/assistant/store";
import type { AssistantRow, RowAction } from "@/modules/assistant/rows";
import { useAgentProvidersStore } from "@/modules/chat/providers";
import { cn } from "@/lib/utils";
import SettingsSection from "./SettingsSection.vue";

const store = useAssistantStore();
const agentProviders = useAgentProvidersStore();

const rechecking = ref(false);
const loading = computed(() => !store.snapshot && !store.error);

/** The order shown while a row is being dragged; `null` shows the saved order. */
const dragOrder = ref<string[] | null>(null);
const rows = computed<AssistantRow[]>(() => {
  if (!dragOrder.value) return store.rows;
  const byId = new Map(store.rows.map((row) => [row.id, row]));
  return dragOrder.value.flatMap((id) => byId.get(id) ?? []);
});

/** `ids` with `id` moved to `to`, or `null` when that is no move at all. */
function moved(ids: string[], id: string, to: number): string[] | null {
  const from = ids.indexOf(id);
  if (from < 0 || to < 0 || to >= ids.length || from === to) return null;
  const next = [...ids];
  next.splice(to, 0, ...next.splice(from, 1));
  return next;
}

/** The row being dragged; the drag reorders `dragOrder`, which is saved on drop. */
const dragging = ref<string | null>(null);
const DRAG_TYPE = "application/x-orso-assistant";

function onDragStart(event: DragEvent, id: string) {
  dragging.value = id;
  dragOrder.value = rows.value.map((row) => row.id);
  event.dataTransfer?.setData(DRAG_TYPE, id);
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
}

/** Reorder live as the dragged row passes over another, so the drop spot is always visible. */
function onDragOver(event: DragEvent, index: number) {
  if (!dragging.value || !dragOrder.value) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  const next = moved(dragOrder.value, dragging.value, index);
  if (next) dragOrder.value = next;
}

function onDragEnd(event: DragEvent) {
  const next = dragOrder.value;
  const saved = store.rows.map((row) => row.id);
  // A cancelled drag (Escape, or dropped outside the list) puts the saved order back.
  if (event.dataTransfer?.dropEffect !== "none" && next && next.join() !== saved.join()) {
    void store.setOrder(next);
  }
  dragging.value = null;
  dragOrder.value = null;
}

function onKeyMove(event: KeyboardEvent, id: string, delta: number) {
  if (!event.altKey) return;
  event.preventDefault();
  const ids = rows.value.map((row) => row.id);
  const next = moved(ids, id, ids.indexOf(id) + delta);
  if (next) void store.setOrder(next);
  // The row moved in the DOM; keep focus on its handle.
  requestAnimationFrame(() =>
    (document.querySelector(`[data-testid="assistant-handle-${id}"]`) as HTMLElement | null)?.focus(),
  );
}

onMounted(() => {
  if (!store.snapshot) void store.load();
});

async function recheck() {
  rechecking.value = true;
  try {
    await store.load();
  } finally {
    rechecking.value = false;
  }
}

/** The sign-in step to show under a row, if one is waiting on the user. */
function panelFor(id: string): ProviderSignInState | null {
  const state = store.signInFor(id);
  if (!state) return null;
  return state.state === "browser" || state.state === "code" || state.state === "failed"
    ? state
    : null;
}

/** Never rendered: the `v-if` above the panel guards it, this only keeps the prop non-null. */
const NO_SIGN_IN: ProviderSignInState = { state: "cancelled" };

function onAction(row: AssistantRow, action: RowAction) {
  switch (action) {
    case "download":
    case "update":
      // An update uses the same download route until updates get their own.
      void store.download(row.id);
      break;
    case "retry":
      void store.retry(row.id);
      break;
    case "cancel":
      void store.cancel(row.id);
      break;
    case "signIn":
      void store.signIn(row.id);
      break;
    case "connect":
      // There is no chat to open from settings; the row is already ready.
      break;
    case "none":
      break;
  }
}
</script>

<template>
  <SettingsSection label="Assistants" icon="i-hugeicons-chat-bot" data-testid="assistant-settings">
    <template #actions>
      <UButton
        color="neutral"
        variant="outline"
        size="sm"
        icon="i-hugeicons-refresh"
        loading-icon="i-hugeicons-refresh"
        label="Recheck"
        :loading="rechecking"
        :disabled="rechecking"
        data-testid="assistant-settings-recheck"
        @click="recheck"
      />
    </template>

    <p
      v-if="loading"
      class="text-muted flex items-center gap-2 p-4 text-sm"
      data-testid="assistant-settings-loading"
    >
      <UIcon name="i-hugeicons-loading-03" class="size-3.5 animate-spin" /> Checking your setup…
    </p>

    <div v-else-if="store.error && !store.snapshot" class="p-4">
      <UAlert
        color="error"
        variant="subtle"
        icon="i-hugeicons-alert-circle"
        description="We couldn't check your setup just now. Make sure orso is still running, then check again."
        data-testid="assistant-settings-error"
      >
        <template #actions>
          <UButton
            color="neutral"
            variant="outline"
            size="sm"
            icon="i-hugeicons-refresh"
            loading-icon="i-hugeicons-refresh"
            label="Check again"
            :loading="rechecking"
            :disabled="rechecking"
            data-testid="assistant-settings-retry"
            @click="recheck"
          />
        </template>
      </UAlert>
    </div>

    <UEmpty
      v-else-if="!rows.length"
      variant="naked"
      icon="i-hugeicons-chat-bot"
      title="No assistants found"
      description="Download an assistant to make changes to your site, then check again."
      data-testid="assistant-settings-empty"
    >
      <template #actions>
        <UButton
          color="neutral"
          variant="outline"
          size="sm"
          icon="i-hugeicons-refresh"
          loading-icon="i-hugeicons-refresh"
          label="Check again"
          :loading="rechecking"
          :disabled="rechecking"
          @click="recheck"
        />
      </template>
    </UEmpty>

    <ul v-else class="divide-y divide-default [&>li]:px-4" data-testid="assistant-settings-rows">
      <template v-for="(row, index) in rows" :key="row.id">
        <ProviderRow
          :row="row"
          :is-default="row.id === agentProviders.preferred"
          hide-connected-action
          draggable="true"
          :class="cn(dragging === row.id && 'opacity-40')"
          @action="onAction(row, $event)"
          @dragstart="onDragStart($event, row.id)"
          @dragover="onDragOver($event, index)"
          @drop.prevent
          @dragend="onDragEnd"
          @keydown.up="onKeyMove($event, row.id, -1)"
          @keydown.down="onKeyMove($event, row.id, 1)"
        >
          <template #leading>
            <UButton
              color="neutral"
              variant="ghost"
              size="xs"
              square
              icon="i-hugeicons-drag-drop-vertical"
              class="text-dimmed -ml-1.5 cursor-grab active:cursor-grabbing"
              :aria-label="`Move ${row.name}`"
              aria-description="Drag, or press Alt with the up or down arrow, to change your order"
              aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown"
              :data-testid="`assistant-handle-${row.id}`"
            />
          </template>
        </ProviderRow>
        <li v-if="panelFor(row.id)" class="pb-3">
          <SignInPanel
            :provider-name="row.name"
            :state="panelFor(row.id) ?? NO_SIGN_IN"
            @submit-code="store.submitCode(row.id, $event)"
            @cancel="store.cancelSignIn(row.id)"
            @retry="store.signIn(row.id)"
            @dismiss="store.clearSignIn(row.id)"
          />
        </li>
      </template>
    </ul>
    <p
      v-if="rows.length > 1"
      class="text-muted px-4 py-3 text-xs"
      data-testid="assistant-settings-order-hint"
    >
      Drag to set your order. New chats start with the first connected assistant.
    </p>
  </SettingsSection>
</template>
