<script setup lang="ts">
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import { errorMessage } from "@/lib/errors";
import { useChangesStore } from "@/modules/changes/store";
import { useGithubStore } from "@/modules/github/store";

const props = defineProps<{ projectId: string }>();
const changes = useChangesStore();
const github = useGithubStore();

async function onShare() {
  if (!changes.active) return;
  try {
    await changes.share(props.projectId, changes.active.branch);
  } catch (e) {
    if (!changes.needsSignIn) {
      toast.error(errorMessage(e));
    }
  }
}
</script>

<template>
  <div class="flex items-center gap-2">
    <Button
      v-if="changes.active"
      variant="default"
      size="sm"
      class="h-7 px-2 text-xs"
      data-testid="share-preview"
      :disabled="changes.sharing"
      @click="onShare"
    >
      {{ changes.sharing ? "Sharing…" : "Share preview" }}
    </Button>

    <a
      v-if="changes.prUrl"
      class="text-muted-foreground hover:text-foreground truncate text-xs underline-offset-2 hover:underline"
      data-testid="pr-link"
      :href="changes.prUrl"
      target="_blank"
      rel="noopener noreferrer"
    >
      Open PR
    </a>

    <Button
      v-if="changes.needsSignIn"
      variant="outline"
      size="sm"
      class="h-7 px-2 text-xs"
      data-testid="share-sign-in"
      @click="github.signIn()"
    >
      Sign in with GitHub
    </Button>
  </div>
</template>
