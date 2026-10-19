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
import { errorMessage } from "@orso/utils";
import DeviceCodeDialog from "./DeviceCodeDialog.vue";
import { useGithubStore } from "./store";

/** `rail`: a larger avatar, an icon-only sign-in, and a menu opening to the right, for the project sidebar. */
defineProps<{ rail?: boolean }>();

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
        <Button variant="ghost" size="icon" class="rounded-full" :class="rail ? 'size-9' : 'size-7'" :title="gh.user.name || gh.user.login" data-testid="github-account">
          <img :src="gh.user.avatarUrl" alt="" class="rounded-full" :class="rail ? 'size-7' : 'size-5'" />
          <span class="sr-only">{{ gh.user.name || gh.user.login }}</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent :side="rail ? 'right' : 'bottom'" align="end">
        <template v-if="gh.source === 'cli'">
          <DropdownMenuItem disabled>Signed in with GitHub CLI (gh)</DropdownMenuItem>
          <DropdownMenuItem v-if="gh.deviceFlow" data-testid="github-sign-in" @select="gh.signIn()">
            <LogIn /> Sign in with GitHub for pull requests
          </DropdownMenuItem>
        </template>
        <DropdownMenuItem v-else @select="signOut"><LogOut /> Sign out</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
    <Button v-else-if="gh.deviceFlow && rail" variant="ghost" size="icon" class="size-9" title="Sign in with GitHub" aria-label="Sign in with GitHub" data-testid="github-sign-in" @click="gh.signIn()">
      <LogIn />
    </Button>
    <Button v-else-if="gh.deviceFlow" variant="outline" size="sm" data-testid="github-sign-in" @click="gh.signIn()">
      <LogIn /> Sign in with GitHub
    </Button>
    <DeviceCodeDialog />
  </template>
</template>
