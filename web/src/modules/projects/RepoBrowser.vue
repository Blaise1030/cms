<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { errorMessage } from "@orso/utils";
import { useGithubStore } from "@/modules/github/store";
import { projectPath } from "@/router";
import { relativeTime } from "@/lib/relativeTime";
import { useProjectsStore } from "./store";

const gh = useGithubStore();
const projects = useProjectsStore();
const router = useRouter();

const filter = ref("");
const opening = ref<string | null>(null);
const loaded = ref(false);
const error = ref<string | null>(null);

const shown = computed(() => {
  const q = filter.value.trim().toLowerCase();
  return gh.repos.filter((r) => !q || r.fullName.toLowerCase().includes(q));
});

async function load() {
  error.value = null;
  try {
    await gh.loadRepos();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loaded.value = true;
  }
}

async function open(fullName: string) {
  if (opening.value) return;
  const local = projects.projects.find((p) => p.repo === fullName);
  if (local) {
    await router.push(projectPath(local.id));
    return;
  }
  opening.value = fullName;
  error.value = null;
  try {
    const created = await projects.cloneRepo(fullName);
    await router.push(projectPath(created.id));
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    opening.value = null;
  }
}

onMounted(() => {
  if (gh.user) void load();
  else gh.load().catch(() => {});
});

watch(
  () => gh.user,
  (u) => {
    if (u) void load();
  },
);
</script>

<template>
  <section class="flex flex-col gap-3" data-testid="repo-browser">
    <h2 class="text-lg font-medium">Open a project</h2>
    <div v-if="!gh.user" class="flex flex-col items-start gap-2">
      <template v-if="gh.deviceFlow">
        <Button size="sm" data-testid="repo-sign-in" @click="gh.signIn()">Sign in with GitHub</Button>
      </template>
      <p v-else class="text-sm">
        Run <code class="bg-muted rounded px-1">gh auth login</code> in a terminal to see your repositories.
      </p>
    </div>
    <template v-else>
      <Input v-model="filter" placeholder="Search repositories by name" aria-label="Search repositories" />
      <div class="divide-y rounded-lg border" data-testid="repo-list">
        <div
          v-if="!loaded || gh.reposLoading"
          class="text-muted-foreground flex items-center gap-2 px-4 py-2.5 text-sm"
          data-testid="repo-loading"
        >
          <Spinner class="size-4" />
          Loading repositories…
        </div>
        <div v-for="r in shown" :key="r.fullName" class="flex items-center gap-3 px-4 py-2.5" data-testid="repo-item">
          <span class="min-w-0 flex-1 truncate text-sm">{{ r.fullName }}</span>
          <span v-if="relativeTime(r.pushedAt)" class="text-muted-foreground text-xs">
            {{ relativeTime(r.pushedAt) }}
          </span>
          <Button variant="outline" size="sm" :disabled="opening !== null" @click="open(r.fullName)">
            <Spinner v-if="opening === r.fullName" class="size-3.5" />
            <template v-else>Open</template>
          </Button>
        </div>
        <p
          v-if="loaded && !gh.reposLoading && shown.length === 0"
          class="text-muted-foreground px-4 py-2.5 text-sm"
        >
          {{ filter.trim() ? "No repositories match." : "No repositories yet." }}
        </p>
      </div>
      <p v-if="error" class="text-destructive text-sm" role="alert">
        {{ error }} <Button variant="link" size="sm" @click="load()">Retry</Button>
      </p>
    </template>
  </section>
</template>
