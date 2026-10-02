<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Loader2 } from "@lucide/vue";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemTitle,
} from "@/components/ui/item";
import { errorMessage } from "@/lib/errors";
import { useGithubStore } from "@/modules/github/store";
import { useProjectsStore } from "./store";

const open = defineModel<boolean>("open", { default: false });
const gh = useGithubStore();
const projects = useProjectsStore();

const filter = ref("");
const opening = ref<string | null>(null);
const error = ref<string | null>(null);

function loadRepos() {
  gh.loadRepos().catch((e) => (error.value = errorMessage(e)));
}

watch(open, (isOpen) => {
  if (!isOpen) return;
  filter.value = "";
  error.value = null;
  if (gh.user) loadRepos();
  // Signed out: the user may have just run `gh auth login`; re-check (the gh.user watch loads repos).
  else gh.load().catch(() => {});
});

// Right after signing in from this dialog, fetch the list.
watch(
  () => gh.user,
  (u) => {
    if (open.value && u) loadRepos();
  },
);

// Don't let the dialog close mid-clone: a failure would otherwise go unseen.
function onOpenChange(v: boolean) {
  if (!v && opening.value) return;
  open.value = v;
}

const shown = computed(() => {
  const q = filter.value.trim().toLowerCase();
  return gh.repos.filter((r) => !q || r.fullName.toLowerCase().includes(q));
});

async function openRepo(fullName: string) {
  if (opening.value) return;
  opening.value = fullName;
  error.value = null;
  try {
    await projects.cloneRepo(fullName);
    open.value = false;
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    opening.value = null;
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="onOpenChange">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle>Open from GitHub</DialogTitle>
        <DialogDescription>
          Pick a site repository. It's downloaded to this computer so you can edit and preview it.
        </DialogDescription>
      </DialogHeader>
      <div v-if="!gh.user" class="flex flex-col items-start gap-2">
        <template v-if="gh.deviceFlow">
          <p class="text-sm">Sign in with GitHub to see your repositories.</p>
          <Button size="sm" data-testid="open-sign-in" @click="gh.signIn()">Sign in with GitHub</Button>
        </template>
        <p v-else class="text-sm">
          Run <code class="bg-muted rounded px-1">gh auth login</code> in a terminal, then reopen this dialog.
        </p>
      </div>
      <template v-else>
        <Input v-model="filter" placeholder="Filter repositories…" aria-label="Filter repositories" />
        <div class="max-h-72 overflow-y-auto">
          <p v-if="gh.reposLoading" class="text-muted-foreground p-2 text-sm">Loading…</p>
          <ItemGroup v-else>
            <Item
              v-for="r in shown"
              :key="r.fullName"
              as="button"
              type="button"
              size="sm"
              class="hover:bg-accent/60 w-full text-left disabled:pointer-events-none disabled:opacity-50"
              :aria-busy="opening === r.fullName"
              :disabled="!!opening"
              data-testid="repo-item"
              @click="openRepo(r.fullName)"
            >
              <ItemContent class="min-w-0">
                <ItemTitle class="w-full"><span class="truncate">{{ r.fullName }}</span></ItemTitle>
                <ItemDescription class="text-xs">
                  {{ r.private ? "Private" : "Public" }}{{ r.canPush ? "" : " · read-only" }}
                </ItemDescription>
              </ItemContent>
              <ItemActions>
                <Loader2 v-if="opening === r.fullName" class="size-4 animate-spin" />
              </ItemActions>
            </Item>
            <p v-if="shown.length === 0 && !error" class="text-muted-foreground p-2 text-sm">
              {{ filter.trim() ? "No repositories match." : "No repositories found." }}
            </p>
          </ItemGroup>
        </div>
      </template>
      <p v-if="error" class="text-destructive text-sm" role="alert">{{ error }}</p>
    </DialogContent>
  </Dialog>
</template>
