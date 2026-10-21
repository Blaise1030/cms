<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { RouterLink, useRouter } from "vue-router";
import { EllipsisVertical, Trash2 } from "@lucide/vue";
import { toast } from "vue-sonner";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Skeleton } from "@/components/ui/skeleton";
import PawMark from "@/app/PawMark.vue";
import type { Project } from "@/core/types";
import { isMacDesktop } from "@/lib/desktop";
import { errorMessage } from "@orso/utils";
import GithubAccount from "@/modules/github/GithubAccount.vue";
import NewProjectDialog from "./NewProjectDialog.vue";
import RepoBrowser from "./RepoBrowser.vue";
import TemplateGallery from "./TemplateGallery.vue";
import { STATUS_LABEL } from "./status";
import { useProjectsStore } from "./store";
import { projectPath } from "@/router";

const emit = defineEmits<{ deleted: [id: string] }>();
const router = useRouter();
const store = useProjectsStore();
const macDesktop = isMacDesktop();

/** Newest first, capped: the daemon and the store append new projects to the end. */
const RECENT_LIMIT = 5;
const recent = computed(() => [...store.projects].reverse().slice(0, RECENT_LIMIT));

const STATUS_DOT: Record<Project["devserver"], string> = {
  starting: "bg-amber-500 animate-pulse",
  ready: "bg-emerald-500",
  crashed: "bg-red-500",
};

const showNew = ref(false);
const pendingDelete = ref<Project | null>(null);
const deleting = ref(false);

onMounted(() => {
  if (!store.loaded) void store.load();
});

async function confirmDelete() {
  const target = pendingDelete.value;
  if (!target) return;
  deleting.value = true;
  try {
    await store.remove(target.id);
    emit("deleted", target.id);
    toast.success(`Deleted ${target.name}`);
  } catch (e) {
    toast.error(`Could not delete ${target.name}: ${errorMessage(e)}`);
  } finally {
    deleting.value = false;
    pendingDelete.value = null;
  }
}

function open(project: Project) {
  void router.push(projectPath(project.id));
}
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- One title-bar row. On macOS it is the drag region and clears the traffic lights. -->
    <div
      class="flex h-11 shrink-0 items-start justify-between pt-1"
      :class="macDesktop ? 'pl-20 pr-3 fullscreen:hidden' : 'px-3'"
      :data-tauri-drag-region="macDesktop ? '' : undefined"
      data-testid="page-header"
    >
      <PawMark class="size-7" />
      <div data-testid="account-slot"><GithubAccount /></div>
    </div>
    <div class="relative min-h-0 flex-1 overflow-y-auto" data-testid="projects-scroll">
      <div class="mx-auto flex w-full max-w-3xl flex-col gap-12 px-10 pt-16 pb-16">
        <section class="flex flex-col gap-3">
          <h2 class="text-lg font-medium">Recent</h2>

          <p v-if="store.error" class="text-destructive text-sm" role="alert">
            {{ store.error }}
            <Button variant="link" size="sm" @click="store.load()">Retry</Button>
          </p>

          <div v-if="store.loading && !store.loaded" class="flex flex-col gap-2">
            <Skeleton v-for="n in 3" :key="n" class="h-14 w-full" />
          </div>

          <p
            v-else-if="store.loaded && store.projects.length === 0"
            class="text-muted-foreground text-sm"
          >
            No projects yet.
          </p>

          <div v-else class="divide-y overflow-hidden rounded-lg border" data-testid="project-grid">
            <div
              v-for="project in recent"
              :key="project.id"
              :data-testid="`project-${project.id}`"
              class="flex items-center gap-3 px-4 py-2.5"
            >
              <span :title="STATUS_LABEL[project.devserver]" data-testid="project-status">
                <span class="block size-2 rounded-full" :class="STATUS_DOT[project.devserver]" />
                <span class="sr-only">{{ STATUS_LABEL[project.devserver] }}</span>
              </span>
              <span class="min-w-0 flex-1">
                <span class="block truncate text-sm">{{ project.name }}</span>
                <span v-if="project.repo" class="text-muted-foreground block truncate text-xs">
                  {{ project.repo }}
                </span>
              </span>
              <Button variant="outline" size="sm" as-child>
                <RouterLink :to="projectPath(project.id)" :aria-label="`Open ${project.name}`">
                  Open
                </RouterLink>
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger as-child>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    :aria-label="`Actions for ${project.name}`"
                    @keydown.enter.stop
                  >
                    <EllipsisVertical />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem variant="destructive" @select="pendingDelete = project">
                    <Trash2 /> Delete
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </section>
        <RepoBrowser />
        <TemplateGallery @create="showNew = true" />
      </div>
    </div>
  </div>

  <NewProjectDialog v-model:open="showNew" @created="open" />

  <AlertDialog :open="pendingDelete !== null" @update:open="(o) => !o && (pendingDelete = null)">
    <AlertDialogContent>
      <AlertDialogHeader>
        <AlertDialogTitle>Delete {{ pendingDelete?.name }}?</AlertDialogTitle>
        <AlertDialogDescription>
          This stops the dev server and permanently removes the project's files, including its
          checkpoints. This cannot be undone.
        </AlertDialogDescription>
      </AlertDialogHeader>
      <AlertDialogFooter>
        <AlertDialogCancel>Cancel</AlertDialogCancel>
        <Button variant="destructive" :disabled="deleting" @click="confirmDelete">
          Delete project
        </Button>
      </AlertDialogFooter>
    </AlertDialogContent>
  </AlertDialog>
</template>
