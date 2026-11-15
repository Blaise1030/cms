<script setup lang="ts">
// App settings → About. Small and factual: the running version, where the data
// lives, and a way out to the project.
import { onMounted, ref } from "vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import ULink from "@nuxt/ui/components/Link.vue";
import { useDaemon } from "@/core/daemon";
import SettingsRow from "./SettingsRow.vue";
import SettingsSection from "./SettingsSection.vue";

const REPO_URL = "https://github.com/Blaise1030/cms";
const DOCS_URL = `${REPO_URL}/tree/main/docs`;

const daemon = useDaemon();
const version = ref<string | null>(null);
const unavailable = ref(false);

onMounted(async () => {
  try {
    const health = await daemon.api.health();
    version.value = health.version;
  } catch {
    unavailable.value = true;
  }
});
</script>

<template>
  <SettingsSection label="About" icon="i-hugeicons-information-circle" data-testid="about-settings">
    <SettingsRow
      icon="i-hugeicons-information-circle"
      title="Version"
      description="The version of orso running on this Mac."
    >
      <template #actions>
        <UBadge color="neutral" variant="subtle" data-testid="about-version">
          {{ version ? `v${version.replace(/^v/, "")}` : unavailable ? "Unknown" : "Checking…" }}
        </UBadge>
      </template>
    </SettingsRow>

    <SettingsRow
      icon="i-hugeicons-hard-drive"
      title="Your data stays local"
      description="Sites, content, and history live on this Mac. Nothing is uploaded unless you publish or share."
    />

    <SettingsRow icon="i-hugeicons-link-square-02" title="Links">
      <template #description>
        <ULink :to="DOCS_URL" target="_blank" class="text-highlighted underline underline-offset-4">
          Documentation
        </ULink>
        and
        <ULink :to="REPO_URL" target="_blank" class="text-highlighted underline underline-offset-4">
          orso on GitHub
        </ULink>.
      </template>
    </SettingsRow>
  </SettingsSection>
</template>
