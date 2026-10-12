<script setup lang="ts">
import { ExternalLink, GitPullRequest, Loader2, LogIn, Share2 } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import { errorMessage } from "@/lib/errors";
import ProtectionHint from "@/modules/changes/ProtectionHint.vue";
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
  <div class="relative flex items-center gap-2">
    <Button
      v-if="changes.active"
      variant="default"
      size="sm"
      class="h-7 gap-1.5 px-2.5 text-xs"
      data-testid="share-preview"
      :disabled="changes.sharing"
      @click="onShare"
    >
      <Loader2 v-if="changes.sharing" class="size-3.5 animate-spin" />
      <Share2 v-else class="size-3.5" />
      {{ changes.sharing ? "Sharing…" : "Share preview" }}
    </Button>

    <a
      v-if="changes.prUrl"
      class="text-muted-foreground hover:text-foreground flex items-center gap-1 truncate text-xs underline-offset-2 hover:underline"
      data-testid="pr-link"
      :href="changes.prUrl"
      target="_blank"
      rel="noopener noreferrer"
    >
      <GitPullRequest class="size-3.5 shrink-0" />
      Open PR
      <ExternalLink class="size-3 shrink-0" />
    </a>

    <Button
      v-if="changes.needsSignIn"
      variant="outline"
      size="sm"
      class="h-7 gap-1.5 px-2 text-xs"
      data-testid="share-sign-in"
      @click="github.signIn()"
    >
      <LogIn class="size-3.5" />
      Sign in with GitHub
    </Button>

    <ProtectionHint :project-id="projectId" />
  </div>
</template>
