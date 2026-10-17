<script setup lang="ts">
import { computed, ref } from "vue";
import { CircleSlash, GitCommitVertical, RotateCcw } from "@lucide/vue";
import { toast } from "vue-sonner";
import { useDaemon } from "@/core/daemon";
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
import { Marker, MarkerContent, MarkerIcon } from "@/components/ui/marker";
import { errorMessage } from "@workbench/utils";
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
  <Marker v-if="checkpoint.blocked" data-testid="checkpoint">
    <MarkerIcon><CircleSlash /></MarkerIcon>
    <MarkerContent>Not saved: {{ checkpoint.blocked }}</MarkerContent>
  </Marker>
  <Marker v-else-if="checkpoint.sha" data-testid="checkpoint">
    <MarkerIcon><GitCommitVertical /></MarkerIcon>
    <MarkerContent :title="checkpoint.files.join('\n')">{{ saved }}</MarkerContent>
    <Button variant="ghost" size="xs" :disabled="restoring" @click="confirming = true">
      <RotateCcw />
      Restore
    </Button>

    <AlertDialog v-model:open="confirming">
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Restore this checkpoint?</AlertDialogTitle>
          <AlertDialogDescription>
            The project's files go back to {{ short }}. Later checkpoints and any uncommitted changes are
            discarded.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <Button variant="destructive" :disabled="restoring" data-testid="confirm-restore" @click="restore">
            {{ restoring ? "Restoring…" : "Restore" }}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  </Marker>
</template>
