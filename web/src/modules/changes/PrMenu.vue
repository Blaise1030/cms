<script setup lang="ts">
import { computed, type Component } from "vue";
import { toast } from "vue-sonner";
import { BadgeCheck, ChevronDown, CircleCheck, CircleDashed, CircleDot, CircleX, Clock, ExternalLink, GitPullRequest, Globe } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { ChecksState, PrState, PrStatus } from "@/core/types";
import { errorMessage } from "@orso/utils";
import { prStatusLabel } from "./prStatus";
import { useChangesStore } from "./store";

defineProps<{ variant: "default" | "secondary" }>();

const store = useChangesStore();
const status = computed(() => store.status);
/** The deployed preview, only if it is a plain web URL (never `javascript:` and the like). */
const previewHref = computed(() => {
  const url = status.value?.previewUrl;
  return url && /^https?:\/\/./i.test(url) ? url : null;
});

const CHECK_ICON: Record<Exclude<ChecksState, "none">, { icon: Component; class: string }> = {
  pending: { icon: Clock, class: "text-amber-500 animate-pulse" },
  passing: { icon: CircleCheck, class: "text-emerald-500" },
  failing: { icon: CircleX, class: "text-red-500" },
};

const STATE_ICON: Record<PrState, { icon: Component; class: string }> = {
  draft: { icon: CircleDashed, class: "text-muted-foreground" },
  open: { icon: CircleDot, class: "text-sky-500" },
  approved: { icon: BadgeCheck, class: "text-emerald-500" },
  merged: { icon: CircleCheck, class: "text-emerald-500" },
  closed: { icon: CircleX, class: "text-muted-foreground" },
};

/** A terminal PR shows its outcome; an active one shows its checks when it has any. */
function statusIcon(s: Pick<PrStatus, "state" | "checks">) {
  if (s.state === "merged" || s.state === "closed") return STATE_ICON[s.state];
  return s.checks === "none" ? STATE_ICON[s.state] : CHECK_ICON[s.checks];
}
const icon = computed(() => (status.value ? statusIcon(status.value) : null));

/**
 * Route a menu link out to the browser ourselves. Selecting an item closes the menu, which
 * unmounts the anchor before WebKit runs its `target="_blank"` navigation and drops the click
 * (the desktop shell's new-window handler opens whatever `window.open` asks for).
 */
function openExternal(e: MouseEvent) {
  const href = (e.currentTarget as HTMLAnchorElement | null)?.href ?? "";
  if (!/^https?:\/\//i.test(href)) return;
  e.preventDefault();
  window.open(href, "_blank", "noopener,noreferrer");
}

async function onReady() {
  const id = store.projectId;
  const branch = store.active?.branch;
  if (!id || !branch) return;
  try {
    await store.markReady(id, branch);
  } catch (e) {
    // An expired session shows the sign-in prompt instead.
    if (!store.needsSignIn) toast.error(errorMessage(e));
  }
}
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <Button :variant="variant" size="sm" class="h-7 gap-1.5 px-2.5 text-xs" data-testid="pr-menu">
        <GitPullRequest class="size-4 shrink-0" />
        {{ status ? `PR #${status.number}` : "Pull request" }}
        <component
          v-if="icon"
          :is="icon.icon"
          class="size-3.5 shrink-0"
          :class="icon.class"
          data-testid="pr-trigger-icon"
          aria-hidden="true"
        />
        <ChevronDown class="size-3.5 shrink-0 opacity-60" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" class="min-w-56">
      <template v-if="status">
        <DropdownMenuLabel
          class="text-muted-foreground flex items-center gap-2 text-xs font-normal"
          data-testid="pr-status"
          :data-state="status.state"
          :data-checks="status.checks"
        >
          <component
            v-if="icon"
            :is="icon.icon"
            class="size-3.5 shrink-0"
            :class="icon.class"
            data-testid="pr-status-icon"
            aria-hidden="true"
          />
          {{ prStatusLabel(status) }}
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
      </template>
      <DropdownMenuItem as-child>
        <a :href="status?.url ?? store.prUrl ?? undefined" target="_blank" rel="noopener noreferrer" data-testid="pr-link" @click="openExternal">
          <GitPullRequest /> Open on GitHub <ExternalLink class="ml-auto opacity-60" />
        </a>
      </DropdownMenuItem>
      <DropdownMenuItem v-if="previewHref" as-child>
        <a :href="previewHref" target="_blank" rel="noopener noreferrer" data-testid="preview-deploy-link" @click="openExternal">
          <Globe /> Open deployed preview <ExternalLink class="ml-auto opacity-60" />
        </a>
      </DropdownMenuItem>
      <template v-if="status?.state === 'draft'">
        <DropdownMenuSeparator />
        <DropdownMenuItem :disabled="store.markingReady" data-testid="ready-for-review" @select="onReady">
          <CircleCheck /> {{ store.markingReady ? "Marking ready…" : "Ready for review" }}
        </DropdownMenuItem>
      </template>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
