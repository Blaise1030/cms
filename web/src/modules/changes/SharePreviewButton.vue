<script setup lang="ts">
import { computed } from "vue";
import { toast } from "@/lib/toast";
import UButton from "@nuxt/ui/components/Button.vue";
import UFieldGroup from "@nuxt/ui/components/FieldGroup.vue";
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

/** Share starts the PR (solid primary); once it exists, sharing again is the quieter action. */
const shareIcon = computed(() => (changes.prUrl ? "i-hugeicons-refresh" : "i-hugeicons-share-08"));

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
    <UFieldGroup v-if="showGroup" data-testid="share-publish-group">
      <UButton
        v-if="showShare"
        :color="variant === 'default' ? 'primary' : 'neutral'"
        :variant="variant === 'default' ? 'solid' : 'outline'"
        size="sm"
        :icon="shareIcon"
        :loading="changes.sharing"
        :label="label"
        data-testid="share-preview"
        :disabled="changes.sharing"
        @click="onShare"
      />
      <span
        v-if="showShare && (changes.prUrl || showPublish)"
        aria-hidden="true"
        class="w-px self-stretch"
        :class="changes.prUrl ? 'bg-accented' : 'bg-default/25'"
      />
      <PrMenu v-if="changes.prUrl" :variant="variant" />
      <span v-if="changes.prUrl && showPublish" aria-hidden="true" class="bg-accented w-px self-stretch" />
      <PublishButton
        v-if="showPublish"
        :project-id="projectId"
        :repo="repo ?? null"
      />
    </UFieldGroup>

    <UButton
      v-else-if="liveHref"
      color="neutral"
      variant="outline"
      size="sm"
      icon="i-hugeicons-globe-02"
      trailing-icon="i-hugeicons-link-square-02"
      :ui="{ trailingIcon: 'size-3.5 opacity-60' }"
      label="Open live site"
      :to="liveHref"
      target="_blank"
      data-testid="live-site-link"
      @click="openLive"
    />

    <UButton
      v-if="changes.needsSignIn"
      color="neutral"
      variant="outline"
      size="sm"
      icon="i-hugeicons-login-03"
      label="Sign in with GitHub"
      data-testid="share-sign-in"
      @click="github.signIn()"
    />

    <ProtectionHint :project-id="projectId" />
  </div>
</template>
