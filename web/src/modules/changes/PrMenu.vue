<script setup lang="ts">
import { computed } from "vue";
import { toast } from "vue-sonner";
import { ChevronDown, CircleCheck, ExternalLink, GitPullRequest, Globe } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { ChecksState } from "@/core/types";
import { errorMessage } from "@workbench/utils";
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

const DOT: Record<ChecksState, string> = {
  none: "bg-muted-foreground/50",
  pending: "bg-amber-500 animate-pulse",
  passing: "bg-emerald-500",
  failing: "bg-red-500",
};

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
        <span
          v-if="status && status.checks !== 'none'"
          class="size-1.5 shrink-0 rounded-full"
          :class="DOT[status.checks]"
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
          <span class="size-1.5 shrink-0 rounded-full" :class="DOT[status.checks]" aria-hidden="true" />
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
