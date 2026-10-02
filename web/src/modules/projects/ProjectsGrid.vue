<script setup lang="ts">
import { onMounted, ref } from "vue";
import { FolderGit2, Plus, Trash2 } from "@lucide/vue";
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
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import type { Project } from "@/core/types";
import { errorMessage } from "@/lib/errors";
import { useGithubStore } from "@/modules/github/store";
import NewProjectDialog from "./NewProjectDialog.vue";
import OpenRepoDialog from "./OpenRepoDialog.vue";
import { STATUS_LABEL, STATUS_TONE } from "./status";
import { useProjectsStore } from "./store";

const emit = defineEmits<{ open: [id: string]; deleted: [id: string] }>();
const store = useProjectsStore();
const gh = useGithubStore();

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

function open(id: string) {
  store.select(id);
  emit("open", id);
}
</script>

<template>
  <ScrollArea class="h-full">
    <div class="mx-auto flex max-w-4xl flex-col gap-6 p-6">
      <div class="flex items-center justify-between">
        <div>
          <h1 class="text-lg font-semibold">Projects</h1>
          <p class="text-muted-foreground text-sm">Pick a site to keep building, or start a new one.</p>
        </div>
        <div class="flex gap-2">
          <Button v-if="gh.configured" variant="outline" @click="showOpen = true">
            <FolderGit2 /> Open from GitHub
          </Button>
          <Button @click="showNew = true"><Plus /> New project</Button>
        </div>
      </div>

      <p v-if="store.error" class="text-destructive text-sm" role="alert">
        {{ store.error }}
        <Button variant="link" size="sm" @click="store.load()">Retry</Button>
      </p>

      <div v-if="store.loading && !store.loaded" class="grid gap-4 sm:grid-cols-2">
        <Skeleton v-for="n in 2" :key="n" class="h-28 w-full" />
      </div>

      <p
        v-else-if="store.loaded && store.projects.length === 0"
        class="text-muted-foreground rounded-lg border border-dashed p-8 text-center text-sm"
      >
        No projects yet. Create one to get started.
      </p>

      <div v-else class="grid gap-4 sm:grid-cols-2" data-testid="project-grid">
        <Card
          v-for="project in store.projects"
          :key="project.id"
          class="hover:border-ring cursor-pointer transition-colors"
          :data-testid="`project-${project.id}`"
          @click="open(project.id)"
        >
          <CardHeader class="flex flex-row items-start justify-between gap-2">
            <CardTitle class="truncate">{{ project.name }}</CardTitle>
            <Badge :variant="STATUS_TONE[project.devserver]" data-testid="project-status">
              {{ STATUS_LABEL[project.devserver] }}
            </Badge>
          </CardHeader>
          <CardContent class="flex items-center justify-between">
            <span class="text-muted-foreground text-xs">port {{ project.port }}</span>
            <Button
              variant="ghost"
              size="icon-sm"
              :aria-label="`Delete ${project.name}`"
              @click.stop="pendingDelete = project"
            >
              <Trash2 />
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  </ScrollArea>

  <NewProjectDialog v-model:open="showNew" />
  <OpenRepoDialog v-model:open="showOpen" />

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
