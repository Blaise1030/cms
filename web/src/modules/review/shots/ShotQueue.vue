<script setup lang="ts">
import { ref } from "vue";
import { useQueryClient } from "@tanstack/vue-query";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import USlideover from "@nuxt/ui/components/Slideover.vue";
import UTextarea from "@nuxt/ui/components/Textarea.vue";
import { useDaemon } from "@/core/daemon";
import { errorMessage } from "@orso/utils";
import { reviewKeys } from "../queries";
import { useShotQueue, type ShotItem } from "./queue";
import { shotItemsForReview } from "./send";

/**
 * The screenshot comments queued on this change: a "Queued (n)" button opening a list to edit,
 * remove, revisit (opens the page it was taken on) and send as one review comment. Give
 * greenlight and Send back also carry the queue.
 */
const props = defineProps<{ projectId: string; number: number; headSha: string; open: boolean }>();
const emit = defineEmits<{ navigate: [item: ShotItem] }>();

const daemon = useDaemon();
const client = useQueryClient();
const queue = useShotQueue(
  () => props.projectId,
  () => props.number,
);
const shown = ref(false);
const sending = ref(false);
const error = ref<string | null>(null);

async function send() {
  if (!queue.items.value.length || sending.value) return;
  sending.value = true;
  error.value = null;
  try {
    await daemon.api.reviewWithShots(props.projectId, props.number, {
      event: "COMMENT",
      body: "",
      headSha: props.headSha,
      items: shotItemsForReview(queue.items.value),
    });
    queue.clear();
    shown.value = false;
    void client.invalidateQueries({ queryKey: reviewKeys.conversation(props.projectId, props.number) });
  } catch (e) {
    // The queue stays, so nothing is lost.
    error.value = errorMessage(e);
  } finally {
    sending.value = false;
  }
}

function revisit(item: ShotItem) {
  emit("navigate", item);
  shown.value = false;
}
</script>

<template>
  <USlideover v-model:open="shown" title="Screenshot comments" :ui="{ body: 'flex flex-col gap-3' }">
    <UButton
      v-if="queue.items.value.length"
      color="neutral"
      variant="soft"
      size="sm"
      icon="i-hugeicons-crop"
      class="self-center"
      :label="`Queued (${queue.items.value.length})`"
      data-testid="shot-queue-open"
    />
    <template #body>
      <p v-if="!queue.items.value.length" class="text-muted text-sm">Nothing queued.</p>
      <article
        v-for="(item, i) in queue.items.value"
        :key="item.id"
        class="border-default flex flex-col gap-2 rounded-lg border p-2"
        data-testid="shot-queue-item"
      >
        <button
          v-if="item.imageDataUrl"
          type="button"
          class="block overflow-hidden rounded-md"
          :aria-label="`Open ${item.path} on ${item.side === 'live' ? 'the live site' : 'this change'}`"
          data-testid="shot-queue-revisit"
          @click="revisit(item)"
        >
          <img :src="item.imageDataUrl" alt="" class="max-h-40 w-full object-contain" />
        </button>
        <UTextarea
          :model-value="item.note"
          :rows="2"
          autoresize
          :maxrows="6"
          placeholder="Add a note…"
          :aria-label="`Note ${i + 1}`"
          data-testid="shot-queue-note"
          @update:model-value="queue.update(item.id, { note: String($event) })"
        />
        <div class="flex items-center gap-2 text-xs">
          <button
            type="button"
            class="text-muted min-w-0 flex-1 truncate text-left hover:underline"
            data-testid="shot-queue-page"
            @click="revisit(item)"
          >
            <code>{{ item.path }}</code> · {{ item.side === "live" ? "Live site" : "This change" }} · {{ item.viewport }}
          </button>
          <UButton
            color="neutral"
            variant="ghost"
            size="xs"
            icon="i-hugeicons-delete-02"
            :aria-label="`Remove comment ${i + 1}`"
            data-testid="shot-queue-remove"
            @click="queue.remove(item.id)"
          />
        </div>
      </article>
    </template>
    <template #footer>
      <div class="flex w-full flex-col gap-2">
        <UAlert v-if="error" color="error" variant="subtle" icon="i-hugeicons-alert-circle" :description="error" data-testid="shot-queue-error" />
        <p class="text-muted text-xs">Give greenlight and Send back include these too.</p>
        <UButton
          block
          label="Send comments"
          icon="i-hugeicons-sent"
          :loading="sending"
          :disabled="!open || !queue.items.value.length || sending"
          data-testid="shot-queue-send"
          @click="send"
        />
      </div>
    </template>
  </USlideover>
</template>
