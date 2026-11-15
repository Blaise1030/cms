<script setup lang="ts">
import { computed, ref } from "vue";
import { toast } from "@/lib/toast";
import UButton from "@nuxt/ui/components/Button.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import USeparator from "@nuxt/ui/components/Separator.vue";
import { useDaemon } from "@/core/daemon";
import { errorMessage } from "@orso/utils";
import type { CheckpointData } from "./parts";

const props = defineProps<{ projectId: string; checkpoint: CheckpointData }>();
const daemon = useDaemon();

const confirming = ref(false);
const restoring = ref(false);

const short = computed(() => props.checkpoint.sha?.slice(0, 7) ?? "");
const saved = computed(() => {
  const n = props.checkpoint.files.length;
  return `Saved ${n} ${n === 1 ? "file" : "files"}`;
});

async function restore() {
  const sha = props.checkpoint.sha;
  if (!sha) return;
  restoring.value = true;
  try {
    await daemon.api.restoreCheckpoint(props.projectId, sha);
    toast.success(`Restored ${short.value}`);
  } catch (e) {
    toast.error(`Restore failed: ${errorMessage(e)}`);
  } finally {
    restoring.value = false;
    confirming.value = false;
  }
}
</script>

<template>
  <!-- An inline event row in the transcript: a labelled rule, with Restore at its end. -->
  <div v-if="checkpoint.blocked || checkpoint.sha" data-testid="checkpoint" class="flex items-center gap-2">
    <USeparator
      position="start"
      decorative
      class="min-w-0 flex-1"
      :ui="{ container: 'text-muted min-w-0 items-center gap-1.5 pe-2 font-normal', border: 'border-muted' }"
    >
      <UIcon
        :name="checkpoint.blocked ? 'i-hugeicons-unavailable' : 'i-hugeicons-git-commit'"
        class="size-4 shrink-0"
        aria-hidden="true"
      />
      <span v-if="checkpoint.blocked" class="truncate text-xs">Not saved: {{ checkpoint.blocked }}</span>
      <span v-else class="truncate text-xs" :title="checkpoint.files.join('\n')">{{ saved }}</span>
    </USeparator>
    <UButton
      v-if="!checkpoint.blocked"
      color="neutral"
      variant="ghost"
      size="xs"
      icon="i-hugeicons-arrow-turn-backward"
      label="Restore"
      :disabled="restoring"
      @click="confirming = true"
    />

    <UModal
      v-if="!checkpoint.blocked"
      v-model:open="confirming"
      title="Restore this checkpoint?"
      :description="`The project's files go back to ${short}. Later checkpoints and any uncommitted changes are discarded.`"
      :close="false"
      :dismissible="!restoring"
    >
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="outline" label="Cancel" @click="confirming = false" />
          <UButton
            color="error"
            :label="restoring ? 'Restoring…' : 'Restore'"
            :disabled="restoring"
            data-testid="confirm-restore"
            @click="restore"
          />
        </div>
      </template>
    </UModal>
  </div>
</template>
