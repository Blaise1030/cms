<script setup lang="ts">
import UButton from "@nuxt/ui/components/Button.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UModal from "@nuxt/ui/components/Modal.vue";
import { useGithubStore } from "./store";

const gh = useGithubStore();

function copyCode() {
  if (gh.device) void navigator.clipboard?.writeText(gh.device.userCode).catch(() => {});
}
</script>

<template>
  <UModal
    :open="!!gh.device"
    title="Sign in with GitHub"
    description="Enter this code on GitHub to connect your account. This window closes once you approve."
    :ui="{ content: 'sm:max-w-sm' }"
    @update:open="(open: boolean) => !open && gh.cancel()"
  >
    <template #body>
      <div class="flex flex-col items-center gap-3 py-2">
        <code class="text-highlighted text-2xl font-semibold tracking-widest" data-testid="device-code">
          {{ gh.device?.userCode }}
        </code>
        <UButton
          :to="gh.device?.verificationUri"
          target="_blank"
          rel="noopener"
          trailing-icon="i-hugeicons-link-square-02"
          label="Copy code and open GitHub"
          @click="copyCode"
        />
        <p class="text-muted flex items-center gap-2 text-xs">
          <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" /> Waiting for approval…
        </p>
      </div>
    </template>
  </UModal>
</template>
