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
import { Input } from "@/components/ui/input";
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemTitle,
} from "@/components/ui/item";
import { ScrollArea } from "@/components/ui/scroll-area";
import { errorMessage } from "@/lib/errors";
import { useGithubStore } from "@/modules/github/store";
import { useProjectsStore } from "./store";

const open = defineModel<boolean>("open", { default: false });
const gh = useGithubStore();
const projects = useProjectsStore();

const filter = ref("");
const opening = ref<string | null>(null);
const error = ref<string | null>(null);

watch(open, (isOpen) => {
  if (!isOpen) return;
  filter.value = "";
  error.value = null;
  if (gh.user) gh.loadRepos().catch((e) => (error.value = errorMessage(e)));
});

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
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle>Open from GitHub</DialogTitle>
        <DialogDescription>
          Pick a site repository. It's downloaded to this computer so you can edit and preview it.
        </DialogDescription>
      </DialogHeader>
      <p v-if="!gh.user" class="text-sm">Sign in with GitHub first (top right).</p>
      <template v-else>
        <Input v-model="filter" placeholder="Filter repositories…" aria-label="Filter repositories" />
        <ScrollArea class="max-h-72">
          <p v-if="gh.reposLoading" class="text-muted-foreground p-2 text-sm">Loading…</p>
          <ItemGroup v-else>
            <Item
              v-for="r in shown"
              :key="r.fullName"
              as="button"
              type="button"
              size="sm"
              class="hover:bg-accent/60 w-full text-left"
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
            <p v-if="shown.length === 0" class="text-muted-foreground p-2 text-sm">
              No repositories match.
            </p>
          </ItemGroup>
        </ScrollArea>
      </template>
      <p v-if="error" class="text-destructive text-sm" role="alert">{{ error }}</p>
    </DialogContent>
  </Dialog>
</template>
