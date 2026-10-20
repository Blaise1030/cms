<script setup lang="ts">
import { computed, ref, watch } from "vue";
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
import { Spinner } from "@/components/ui/spinner";
import { errorMessage } from "@orso/utils";
import { useGithubStore } from "@/modules/github/store";
import type { Project } from "@/core/types";
import { useProjectsStore } from "./store";

const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{ created: [project: Project] }>();
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
    const project = await projects.cloneRepo(fullName);
    emit("created", project);
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
        <DialogTitle class="flex items-center gap-2">
          <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true" class="size-4 shrink-0">
            <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z" />
          </svg>
          Open from GitHub
        </DialogTitle>
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
        <div
          class="overflow-y-auto rounded-lg border p-1.5"
          :class="gh.reposLoading ? 'h-72' : 'max-h-72'"
          data-testid="repo-list"
        >
          <div v-if="gh.reposLoading" class="text-muted-foreground flex h-full items-center justify-center gap-2">
            <Spinner class="size-5" />
            <span class="text-sm">Loading repositories…</span>
          </div>
          <ItemGroup v-else class="gap-1.5">
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
                <Spinner v-if="opening === r.fullName" class="size-4" />
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
