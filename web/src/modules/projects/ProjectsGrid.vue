<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { toast } from "@/lib/toast";
import type { DropdownMenuItem } from "@nuxt/ui";
import UButton from "@nuxt/ui/components/Button.vue";
import UCard from "@nuxt/ui/components/Card.vue";
import UChip from "@nuxt/ui/components/Chip.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import USkeleton from "@nuxt/ui/components/Skeleton.vue";
import PawMark from "@/app/PawMark.vue";
import type { Project } from "@/core/types";
import { isMacDesktop } from "@/lib/desktop";
import { errorMessage } from "@orso/utils";
import GithubAccount from "@/modules/github/GithubAccount.vue";
import InviteDialog from "./InviteDialog.vue";
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

const STATUS_COLOR = {
  starting: "warning",
  ready: "success",
  crashed: "error",
} as const satisfies Record<Project["devserver"], string>;

const showNew = ref(false);
const inviteProject = ref<Project | null>(null);
const pendingDelete = ref<Project | null>(null);
const deleting = ref(false);

function actionsFor(project: Project): DropdownMenuItem[] {
  return [
    { label: "Invite", icon: "i-hugeicons-user-add-01", onSelect: () => (inviteProject.value = project) },
    {
      label: "Delete",
      icon: "i-hugeicons-delete-02",
      color: "error",
      onSelect: () => (pendingDelete.value = project),
    },
  ];
}

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
      class="relative z-10 flex h-11 shrink-0 items-start justify-end bg-background pt-1"
      :class="macDesktop ? 'pr-3 fullscreen:hidden' : 'px-3'"
      :data-tauri-drag-region="macDesktop ? '' : undefined"
      data-testid="page-header"
    >
      <div
        class="pointer-events-none absolute inset-x-0 top-full h-6 bg-linear-to-b from-background to-transparent"
        aria-hidden="true"
      />
      <div class="flex items-center gap-2" data-testid="account-slot">
        <GithubAccount />
      </div>
    </div>
    <div class="relative min-h-0 flex-1 overflow-y-auto" data-testid="projects-scroll">
      <div class="mx-auto flex w-full max-w-2xl flex-col gap-8 px-10 pt-16 pb-16">
        <div class="flex flex-col gap-8">
          <header class="mb-2 flex items-center gap-4" data-testid="home-brand">
            <PawMark class="text-muted size-7 shrink-0" data-testid="home-logo" />
            <div class="min-w-0 leading-tight">
              <h1 class="text-highlighted text-lg font-semibold tracking-tight">orso</h1>
              <p class="text-muted text-sm">Build and edit your sites with your assistant</p>
            </div>
          </header>
          <section class="flex flex-col gap-3">
            <h2 class="text-highlighted text-lg font-medium">Recent</h2>

            <p v-if="store.error" class="text-error text-sm" role="alert">
              {{ store.error }}
              <UButton variant="link" size="sm" label="Retry" @click="store.load()" />
            </p>

            <div v-if="store.loading && !store.loaded" class="flex flex-col gap-2">
              <USkeleton v-for="n in 3" :key="n" class="h-14 w-full" />
            </div>

            <p
              v-else-if="store.loaded && store.projects.length === 0"
              class="text-muted text-sm"
            >
              No projects yet.
            </p>

            <UCard
              v-else
              :ui="{ body: 'divide-y divide-default p-0 sm:p-0' }"
              data-testid="project-grid"
            >
              <div
                v-for="project in recent"
                :key="project.id"
                :data-testid="`project-${project.id}`"
                class="flex items-center gap-3 px-4 py-2.5"
              >
                <span class="flex" :title="STATUS_LABEL[project.devserver]" data-testid="project-status">
                  <UChip
                    standalone
                    inset
                    size="md"
                    :color="STATUS_COLOR[project.devserver]"
                    :ui="{ base: project.devserver === 'starting' ? 'animate-pulse' : undefined }"
                  />
                  <span class="sr-only">{{ STATUS_LABEL[project.devserver] }}</span>
                </span>
                <span class="min-w-0 flex-1">
                  <span class="text-highlighted block truncate text-sm">{{ project.name }}</span>
                  <span v-if="project.repo" class="text-muted block truncate text-xs">
                    {{ project.repo }}
                  </span>
                </span>
                <UButton
                  color="neutral"
                  variant="outline"
                  size="sm"
                  label="Open"
                  :to="projectPath(project.id)"
                  :aria-label="`Open ${project.name}`"
                />
                <UDropdownMenu :items="actionsFor(project)" :content="{ align: 'end' }">
                  <UButton
                    color="neutral"
                    variant="ghost"
                    size="sm"
                    icon="i-hugeicons-more-vertical"
                    :aria-label="`Actions for ${project.name}`"
                    @keydown.enter.stop
                  />
                </UDropdownMenu>
              </div>
            </UCard>
          </section>
        </div>
        <RepoBrowser />
        <TemplateGallery @create="showNew = true" />
      </div>
    </div>
  </div>

  <NewProjectDialog v-model:open="showNew" @created="open" />

  <InviteDialog
    v-if="inviteProject"
    :open="inviteProject !== null"
    :name="inviteProject.name"
    :repo="inviteProject.repo"
    @update:open="(o) => !o && (inviteProject = null)"
  />

  <UModal
    :open="pendingDelete !== null"
    :title="`Delete ${pendingDelete?.name}?`"
    description="This stops the dev server and permanently removes the project's files, including its checkpoints. This cannot be undone."
    :close="false"
    @update:open="(o) => !o && (pendingDelete = null)"
  >
    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton color="neutral" variant="outline" label="Cancel" @click="pendingDelete = null" />
        <UButton
          color="error"
          label="Delete project"
          :loading="deleting"
          :disabled="deleting"
          @click="confirmDelete"
        />
      </div>
    </template>
  </UModal>
</template>
