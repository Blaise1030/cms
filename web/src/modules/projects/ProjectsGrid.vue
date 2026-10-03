<script setup lang="ts">
import { onMounted, ref } from "vue";
import { RouterLink, useRouter } from "vue-router";
import { EllipsisVertical, FolderGit2, Plus, Trash2 } from "@lucide/vue";
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
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item";
import { Skeleton } from "@/components/ui/skeleton";
import type { Project } from "@/core/types";
import { errorMessage } from "@/lib/errors";
import GithubAccount from "@/modules/github/GithubAccount.vue";
import { useGithubStore } from "@/modules/github/store";
import NewProjectDialog from "./NewProjectDialog.vue";
import OpenRepoDialog from "./OpenRepoDialog.vue";
import { STATUS_LABEL } from "./status";
import { useProjectsStore } from "./store";
import { projectPath } from "@/router";

const emit = defineEmits<{ deleted: [id: string] }>();
const router = useRouter();
const store = useProjectsStore();
const gh = useGithubStore();

const STATUS_DOT: Record<Project["devserver"], string> = {
  starting: "bg-amber-500 animate-pulse",
  ready: "bg-emerald-500",
  crashed: "bg-red-500",
};

const showNew = ref(false);
const showOpen = ref(false);
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
  <div class="relative h-full overflow-y-auto">
    <div class="absolute top-3 right-3">
      <GithubAccount />
    </div>
    <!-- A welcome page in the spirit of VS Code's: title, start actions, then recent projects. -->
    <div class="flex min-h-full flex-col items-center gap-10 px-10 pt-[12vh] pb-10">
      <header class="text-center">
        <h1 class="text-3xl font-light tracking-tight">workbench</h1>
        <p class="text-muted-foreground mt-1">Build and edit your sites with an AI agent</p>
      </header>

      <div class="flex gap-2">
        <Button @click="showNew = true"><Plus /> New project</Button>
        <Button v-if="gh.configured" variant="outline" @click="showOpen = true">
          <FolderGit2 /> Open from GitHub
        </Button>
      </div>

      <section class="flex w-full max-w-xl flex-col gap-3">
        <h2 class="text-muted-foreground text-sm font-medium">Recent</h2>

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

        <ItemGroup v-else class="gap-2" data-testid="project-grid">
          <!-- The whole row links to the project; the menu stops its clicks from following it. -->
          <Item
            v-for="project in store.projects"
            :key="project.id"
            as-child
            variant="outline"
            size="sm"
          >
            <RouterLink :to="projectPath(project.id)" :data-testid="`project-${project.id}`">
            <ItemMedia>
              <span :title="STATUS_LABEL[project.devserver]" data-testid="project-status">
                <span class="block size-2 rounded-full" :class="STATUS_DOT[project.devserver]" />
                <span class="sr-only">{{ STATUS_LABEL[project.devserver] }}</span>
              </span>
            </ItemMedia>
            <ItemContent class="min-w-0">
              <ItemTitle>{{ project.name }}</ItemTitle>
              <ItemDescription v-if="project.repo">{{ project.repo }}</ItemDescription>
            </ItemContent>
            <ItemActions>
              <Badge variant="outline">localhost:{{ project.port }}</Badge>
              <DropdownMenu>
                <DropdownMenuTrigger as-child>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    :aria-label="`Actions for ${project.name}`"
                    @click.stop.prevent
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
            </ItemActions>
            </RouterLink>
          </Item>
        </ItemGroup>
      </section>
    </div>
  </div>

  <NewProjectDialog v-model:open="showNew" @created="open" />
  <OpenRepoDialog v-model:open="showOpen" @created="open" />

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
