<script setup lang="ts">
// App settings → GitHub. Git and the GitHub tools are optional for local work;
// signing in only matters when a site is shared or published.
import { computed, onMounted, ref } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import DeviceCodeDialog from "@/modules/github/DeviceCodeDialog.vue";
import { useGithubStore } from "@/modules/github/store";
import SettingsRow from "./SettingsRow.vue";
import SettingsSection from "./SettingsSection.vue";

const gh = useGithubStore();

const checking = ref(false);

const status = computed(() => gh.setup);
const githubSignedIn = computed(() => gh.user != null || status.value?.githubSignedIn === true);
const loading = computed(() => !status.value && !gh.setupError);

/** The git and GitHub-tools rows share one shape: ready, or a link to get it. */
const tools = computed(() => [
  {
    id: "git",
    icon: "i-hugeicons-git-branch",
    title: "Git",
    description: "Keeps the history of every change to your sites.",
    ready: status.value?.git === true,
    getLabel: "Get Git",
    getUrl: "https://git-scm.com/downloads/mac",
  },
  {
    id: "tools",
    icon: "i-hugeicons-git-pull-request",
    title: "GitHub tools",
    description: "Connects your sites to GitHub.",
    ready: status.value?.gh === true,
    getLabel: "Get the GitHub tools",
    getUrl: "https://cli.github.com/",
  },
]);

async function recheck() {
  checking.value = true;
  try {
    await gh.loadSetup();
  } finally {
    checking.value = false;
  }
}

onMounted(() => {
  if (!gh.setup) void recheck();
});
</script>

<template>
  <SettingsSection label="Account" icon="i-hugeicons-user-circle" data-testid="github-settings">
    <template #actions>
      <UButton
        color="neutral"
        variant="outline"
        size="sm"
        icon="i-hugeicons-refresh"
        loading-icon="i-hugeicons-refresh"
        label="Recheck"
        :loading="checking"
        :disabled="checking"
        data-testid="github-settings-recheck"
        @click="recheck"
      />
    </template>

    <p
      v-if="loading"
      class="text-muted flex items-center gap-2 p-4 text-sm"
      data-testid="github-settings-loading"
    >
      <UIcon name="i-hugeicons-loading-03" class="size-3.5 animate-spin" /> Checking your setup…
    </p>

    <div v-else-if="gh.setupError || !status" class="p-4">
      <UAlert
        color="error"
        variant="subtle"
        icon="i-hugeicons-alert-circle"
        description="We couldn't check GitHub just now. Make sure orso is still running, then check again."
        data-testid="github-settings-error"
      >
        <template #actions>
          <UButton
            color="neutral"
            variant="outline"
            size="sm"
            icon="i-hugeicons-refresh"
            loading-icon="i-hugeicons-refresh"
            label="Check again"
            :loading="checking"
            :disabled="checking"
            data-testid="github-settings-retry"
            @click="recheck"
          />
        </template>
      </UAlert>
    </div>

    <template v-else>
      <SettingsRow
        v-for="tool in tools"
        :key="tool.id"
        :icon="tool.icon"
        :description="tool.description"
        :data-testid="`github-settings-${tool.id}`"
      >
        <template #title>
          <UIcon
            :name="tool.ready ? 'i-hugeicons-checkmark-circle-02' : 'i-hugeicons-alert-circle'"
            :class="tool.ready ? 'text-success' : 'text-muted'"
            class="size-3.5"
          />
          {{ tool.title }}
        </template>
        <UButton
          v-if="!tool.ready"
          variant="link"
          color="neutral"
          size="sm"
          class="self-start px-0"
          trailing-icon="i-hugeicons-link-square-02"
          :label="tool.getLabel"
          :to="tool.getUrl"
          target="_blank"
        />
        <template #actions>
          <UBadge
            color="neutral"
            :variant="tool.ready ? 'subtle' : 'outline'"
            :label="tool.ready ? 'Ready' : 'Optional'"
          />
        </template>
      </SettingsRow>

      <SettingsRow icon="i-hugeicons-user-circle" data-testid="github-settings-account">
        <template #title>
          <UIcon
            :name="githubSignedIn ? 'i-hugeicons-checkmark-circle-02' : 'i-hugeicons-alert-circle'"
            :class="githubSignedIn ? 'text-success' : 'text-muted'"
            class="size-3.5"
          />
          GitHub account
        </template>
        <template #description>
          <template v-if="githubSignedIn">
            <template v-if="gh.user">Signed in as {{ gh.user.login }}. </template>
            <template v-else>You're signed in to GitHub. </template>
            Share previews and publish when you are ready.
          </template>
          <template v-else>Sign in so orso can open your sites and share your changes.</template>
        </template>

        <template v-if="!githubSignedIn">
          <UButton
            v-if="status.gh && gh.deviceFlow"
            color="neutral"
            variant="outline"
            size="sm"
            class="self-start"
            icon="i-hugeicons-login-03"
            label="Sign in with GitHub"
            data-testid="github-settings-sign-in"
            @click="gh.signIn()"
          />
          <p v-else-if="status.gh" class="text-muted text-xs">
            Sign in with GitHub on this Mac, then select Recheck.
          </p>
          <p v-else class="text-muted text-xs">Install the GitHub tools first, then sign in.</p>
        </template>
        <p v-if="gh.error" class="text-error text-xs" role="alert">
          GitHub sign-in didn't finish. Please try again.
        </p>

        <template #actions>
          <UBadge
            color="neutral"
            :variant="githubSignedIn ? 'subtle' : 'outline'"
            :label="githubSignedIn ? 'Ready' : 'Optional'"
          />
        </template>
      </SettingsRow>
    </template>

    <DeviceCodeDialog />
  </SettingsSection>
</template>
