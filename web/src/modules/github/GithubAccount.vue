<script setup lang="ts">
import { watch } from "vue";
import { LogIn, LogOut } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useDaemon } from "@/core/daemon";
import { errorMessage } from "@/lib/errors";
import DeviceCodeDialog from "./DeviceCodeDialog.vue";
import { useGithubStore } from "./store";

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
</script>

<template>
  <template v-if="gh.configured || gh.user">
    <DropdownMenu v-if="gh.user">
      <DropdownMenuTrigger as-child>
        <Button variant="ghost" size="sm" class="h-7 gap-1.5 px-2 text-xs" data-testid="github-account">
          <img :src="gh.user.avatarUrl" alt="" class="size-5 rounded-full" />
          {{ gh.user.name || gh.user.login }}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem v-if="gh.source === 'cli'" disabled>
          Signed in with GitHub CLI (gh)
        </DropdownMenuItem>
        <DropdownMenuItem v-else @select="signOut"><LogOut /> Sign out</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
    <Button v-else-if="gh.deviceFlow" variant="outline" size="sm" data-testid="github-sign-in" @click="gh.signIn()">
      <LogIn /> Sign in with GitHub
    </Button>
    <DeviceCodeDialog />
  </template>
</template>
