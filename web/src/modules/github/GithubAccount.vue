<script setup lang="ts">
import { computed, watch } from "vue";
import { toast } from "@/lib/toast";
import UAvatar from "@nuxt/ui/components/Avatar.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UDropdownMenu from "@nuxt/ui/components/DropdownMenu.vue";
import type { DropdownMenuItem } from "@nuxt/ui";
import { useDaemon } from "@/core/daemon";
import { errorMessage } from "@orso/utils";
import DeviceCodeDialog from "./DeviceCodeDialog.vue";
import { useGithubStore } from "./store";

/** `rail`: a larger avatar, an icon-only sign-in, and a menu opening to the right, for the project sidebar. */
const props = defineProps<{ rail?: boolean }>();

const gh = useGithubStore();

const daemon = useDaemon();

// Load once the daemon is reachable (and again after a reconnect).
watch(
  daemon.status,
  (s) => {
    if (s !== "open") return;
    gh.load().catch(() => {
      // Older daemons have no /api/github: keep the control hidden.
    });
  },
  { immediate: true },
);

watch(
  () => gh.error,
  (e) => {
    if (e) toast.error(e);
  },
);

async function signOut() {
  try {
    await gh.signOut();
  } catch (e) {
    toast.error(`Sign-out failed: ${errorMessage(e)}`);
  }
}

const menuItems = computed<DropdownMenuItem[]>(() => {
  const items: DropdownMenuItem[] = [];
  if (gh.source === "cli") {
    items.push({ label: "Signed in with GitHub CLI (gh)", disabled: true });
    if (gh.deviceFlow) {
      items.push({
        label: "Sign in with GitHub to share changes",
        icon: "i-hugeicons-login-03",
        "data-testid": "github-sign-in",
        onSelect: () => void gh.signIn(),
      });
    }
  } else {
    items.push({ label: "Sign out", icon: "i-hugeicons-logout-03", onSelect: () => void signOut() });
  }
  items.push({
    label: "Settings",
    icon: "i-hugeicons-settings-01",
    to: "/settings",
    "data-testid": "github-settings",
  });
  return items;
});

const menuContent = computed(() => ({
  side: props.rail ? ("right" as const) : ("bottom" as const),
  align: "end" as const,
}));
</script>

<template>
  <template v-if="gh.configured || gh.user">
    <UDropdownMenu v-if="gh.user" :items="menuItems" :content="menuContent">
      <UButton
        color="neutral"
        variant="ghost"
        square
        class="justify-center rounded-full p-0"
        :class="rail ? 'size-9' : 'size-7'"
        :title="gh.user.name || gh.user.login"
        data-testid="github-account"
      >
        <UAvatar :src="gh.user.avatarUrl" alt="" :size="rail ? 'sm' : '2xs'" />
        <span class="sr-only">{{ gh.user.name || gh.user.login }}</span>
      </UButton>
    </UDropdownMenu>
    <UButton
      v-else-if="gh.deviceFlow && rail"
      color="neutral"
      variant="ghost"
      square
      class="size-9 justify-center"
      icon="i-hugeicons-login-03"
      title="Sign in with GitHub"
      aria-label="Sign in with GitHub"
      data-testid="github-sign-in"
      @click="gh.signIn()"
    />
    <UButton
      v-else-if="gh.deviceFlow"
      color="neutral"
      variant="outline"
      size="sm"
      icon="i-hugeicons-login-03"
      label="Sign in with GitHub"
      data-testid="github-sign-in"
      @click="gh.signIn()"
    />
    <DeviceCodeDialog />
  </template>
</template>
