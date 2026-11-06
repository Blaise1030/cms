<script setup lang="ts">
import { computed } from "vue";
import { ExternalLink, Globe, LogIn, RefreshCw, Share2 } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import { ButtonGroup, ButtonGroupSeparator } from "@/components/ui/button-group";
import { Spinner } from "@/components/ui/spinner";
import { errorMessage } from "@orso/utils";
import PrMenu from "@/modules/changes/PrMenu.vue";
import ProtectionHint from "@/modules/changes/ProtectionHint.vue";
import PublishButton from "@/modules/changes/PublishButton.vue";
import { useChangesStore } from "@/modules/changes/store";
import { useGithubStore } from "@/modules/github/store";

const props = defineProps<{ projectId: string; repo?: string | null }>();
const changes = useChangesStore();
const github = useGithubStore();

/** Once the PR exists, sharing again only pushes new commits to it. */
const variant = computed(() => (changes.prUrl ? "secondary" : "default"));
const label = computed(() => {
  if (changes.prUrl) return changes.sharing ? "Updating…" : "Update preview";
  return changes.sharing ? "Sharing…" : "Share preview";
});

/** Nothing to push once the branch tip is the PR head; hide rather than offer a no-op. */
const showShare = computed(() => changes.active !== null && !changes.status?.upToDate);

/** Publish sits in the same group when this is a GitHub-backed draft that isn't live. */
const showPublish = computed(
  () =>
    props.repo != null &&
    changes.active !== null &&
    changes.status?.state !== "merged",
);

const showGroup = computed(
  () => !!(showShare.value || changes.prUrl || showPublish.value),
);

/** Production deployment of `main`, only as a plain web URL while on the live site. */
const liveHref = computed(() => {
  if (changes.active !== null || props.repo == null) return null;
  const url = changes.liveUrl;
  return url && /^https?:\/\/./i.test(url) ? url : null;
});

/**
 * Open the live site in the desktop shell's browser. Same pattern as PrMenu: `window.open`
 * so the shell's new-window handler gets the URL.
 */
function openLive(e: MouseEvent) {
  const href = (e.currentTarget as HTMLAnchorElement | null)?.href ?? "";
  if (!/^https?:\/\//i.test(href)) return;
  e.preventDefault();
  window.open(href, "_blank", "noopener,noreferrer");
}

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
    <ButtonGroup v-if="showGroup" data-testid="share-publish-group">
      <Button
        v-if="showShare"
        :variant="variant"
        size="sm"
        class="h-7 gap-1.5 px-2.5 text-xs"
        data-testid="share-preview"
        :disabled="changes.sharing"
        @click="onShare"
      >
        <Spinner v-if="changes.sharing" class="size-4" />
        <RefreshCw v-else-if="changes.prUrl" class="size-4" />
        <Share2 v-else class="size-4" />
        {{ label }}
      </Button>
      <ButtonGroupSeparator
        v-if="showShare && (changes.prUrl || showPublish)"
        :class="!changes.prUrl ? 'bg-primary-foreground/25' : undefined"
      />
      <PrMenu v-if="changes.prUrl" :variant="variant" />
      <ButtonGroupSeparator v-if="changes.prUrl && showPublish" />
      <PublishButton
        v-if="showPublish"
        :project-id="projectId"
        :repo="repo ?? null"
      />
    </ButtonGroup>

    <Button
      v-else-if="liveHref"
      as-child
      variant="outline"
      size="sm"
      class="h-7 gap-1.5 px-2.5 text-xs"
    >
      <a
        :href="liveHref"
        target="_blank"
        rel="noopener noreferrer"
        data-testid="live-site-link"
        @click="openLive"
      >
        <Globe class="size-4" />
        Open live site
        <ExternalLink class="size-3.5 opacity-60" />
      </a>
    </Button>

    <Button
      v-if="changes.needsSignIn"
      variant="outline"
      size="sm"
      class="h-7 gap-1.5 px-2 text-xs"
      data-testid="share-sign-in"
      @click="github.signIn()"
    >
      <LogIn class="size-4" />
      Sign in with GitHub
    </Button>

    <ProtectionHint :project-id="projectId" />
  </div>
</template>
