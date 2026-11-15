<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import UButton from "@nuxt/ui/components/Button.vue";
import UCard from "@nuxt/ui/components/Card.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import { errorMessage } from "@orso/utils";
import { useGithubStore } from "@/modules/github/store";
import { projectPath } from "@/router";
import { relativeTime } from "@/lib/relativeTime";
import { openErrorMessage } from "./invite";
import { useProjectsStore } from "./store";

const gh = useGithubStore();
const projects = useProjectsStore();
const router = useRouter();

const filter = ref("");
const opening = ref<string | null>(null);
const loaded = ref(false);
/** Whether the daemon has told us who (if anyone) is signed in. */
const authChecked = ref(gh.user !== null);
const loadError = ref<string | null>(null);
const openError = ref<string | null>(null);

const REPO_LIMIT = 5;

const shown = computed(() => {
  const q = filter.value.trim().toLowerCase();
  return gh.repos.filter((r) => !q || r.fullName.toLowerCase().includes(q)).slice(0, REPO_LIMIT);
});

async function load() {
  // Signing in while the mount-time load is in flight would otherwise fetch twice.
  if (gh.reposLoading) return;
  loadError.value = null;
  try {
    await gh.loadRepos();
  } catch (e) {
    loadError.value = errorMessage(e);
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
  openError.value = null;
  try {
    const created = await projects.cloneRepo(fullName);
    await router.push(projectPath(created.id));
  } catch (e) {
    openError.value = openErrorMessage(e);
  } finally {
    opening.value = null;
  }
}

onMounted(async () => {
  try {
    if (!gh.user) await gh.load();
  } catch {
    // A failed check leaves no user; the prompt then says how to sign in.
  } finally {
    authChecked.value = true;
  }
  if (gh.user) void load();
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
    <h2 class="text-highlighted text-lg font-medium">Open a project</h2>
    <p
      v-if="!authChecked"
      class="text-muted flex items-center gap-2 text-sm"
      data-testid="repo-auth-loading"
      role="status"
    >
      <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" />
      Checking GitHub sign-in…
    </p>
    <div v-else-if="!gh.user" class="flex flex-col items-start gap-2">
      <template v-if="gh.deviceFlow">
        <UButton
          size="sm"
          icon="i-hugeicons-login-03"
          label="Sign in with GitHub"
          data-testid="repo-sign-in"
          @click="gh.signIn()"
        />
      </template>
      <p v-else class="text-default text-sm">
        Run <code class="bg-muted rounded px-1">gh auth login</code> in a terminal to see your repositories.
      </p>
    </div>
    <template v-else>
      <UInput
        v-model="filter"
        class="w-full"
        icon="i-hugeicons-search-01"
        placeholder="Search repositories by name"
        aria-label="Search repositories"
      />
      <UCard :ui="{ body: 'divide-y divide-default p-0 sm:p-0' }" data-testid="repo-list">
        <div
          v-if="!loaded || gh.reposLoading"
          class="text-muted flex items-center gap-2 px-4 py-2.5 text-sm"
          data-testid="repo-loading"
          role="status"
        >
          <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" />
          Loading repositories…
        </div>
        <div v-for="r in shown" :key="r.fullName" class="flex items-center gap-3 px-4 py-2.5" data-testid="repo-item">
          <span class="text-highlighted min-w-0 flex-1 truncate text-sm">{{ r.fullName }}</span>
          <span v-if="relativeTime(r.pushedAt)" class="text-muted text-xs">
            {{ relativeTime(r.pushedAt) }}
          </span>
          <UButton
            color="neutral"
            variant="outline"
            size="sm"
            :label="opening === r.fullName ? undefined : 'Open'"
            :loading="opening === r.fullName"
            :disabled="opening !== null"
            :aria-label="opening === r.fullName ? `Opening ${r.fullName}` : `Open ${r.fullName}`"
            :aria-busy="opening === r.fullName"
            @click="open(r.fullName)"
          />
        </div>
        <p
          v-if="loaded && !gh.reposLoading && !loadError && shown.length === 0"
          class="text-muted px-4 py-2.5 text-sm"
        >
          {{ filter.trim() ? "No repositories match." : "No repositories yet." }}
        </p>
      </UCard>
    </template>
    <!-- Outside the branches above: a rejected token signs the user out, but the failure must stay visible. -->
    <p v-if="loadError" class="text-error text-sm" role="alert">
      {{ loadError }} <UButton variant="link" size="sm" label="Retry" @click="load()" />
    </p>
    <!-- A clone failure is retried by pressing Open again, so no Retry here (it would reload, not clone). -->
    <p v-else-if="openError" class="text-error text-sm" role="alert">{{ openError }}</p>
  </section>
</template>
