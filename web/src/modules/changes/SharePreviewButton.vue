<script setup lang="ts">
import { computed } from "vue";
import { toast } from "@/lib/toast";
import UButton from "@nuxt/ui/components/Button.vue";
import UFieldGroup from "@nuxt/ui/components/FieldGroup.vue";
import UTooltip from "@nuxt/ui/components/Tooltip.vue";
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
const shareIcon = computed(() => (changes.prUrl ? "i-hugeicons-cloud-upload" : "i-hugeicons-share-08"));

/** Nothing to push once the branch tip is the PR head; hide rather than offer a no-op. */
const showShare = computed(() => changes.active !== null && !changes.status?.upToDate);

/**
 * GitHub opens no pull request for a change that doesn't differ from the live site, so the
 * first share waits for an edit. Once a PR exists, updating it stays open.
 */
const shareBlocked = computed(() => !changes.prUrl && !changes.hasEdits);
const blockedReason = "Make an edit first: there's nothing in this change to share yet.";

/** Publish sits beside the group when this is a GitHub-backed draft that isn't live. */
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
    <UFieldGroup v-if="showShare || changes.prUrl" data-testid="share-group">
      <!--
        Blocked only before a pull request exists, so the button is alone in the group then.
        A disabled button gets no pointer events; the span keeps the tooltip working. Unblocked,
        it stays a direct child so the group can join its corners to Review's.
      -->
      <UTooltip v-if="showShare && shareBlocked" :text="blockedReason" :delay-duration="300">
        <span class="inline-flex">
          <UButton
            color="primary"
            size="sm"
            :icon="shareIcon"
            :label="label"
            data-testid="share-preview"
            :aria-description="blockedReason"
            disabled
          />
        </span>
      </UTooltip>
      <UButton
        v-else-if="showShare"
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
      <span v-if="showShare && changes.prUrl" aria-hidden="true" class="bg-accented w-px self-stretch" />
      <PrMenu v-if="changes.prUrl" :variant="variant" />
    </UFieldGroup>

    <!-- Its own button: publishing is a separate step from sharing and reviewing. -->
    <PublishButton v-if="showPublish" :project-id="projectId" :repo="repo ?? null" />

    <UButton
      v-if="!showGroup && liveHref"
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
