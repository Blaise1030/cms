<script setup lang="ts">
import { onMounted, watch } from "vue";
import { LogIn, LogOut } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { errorMessage } from "@/lib/errors";
import DeviceCodeDialog from "./DeviceCodeDialog.vue";
import { useGithubStore } from "./store";

const gh = useGithubStore();

onMounted(() => {
  gh.load().catch(() => {
    // Older daemons have no /api/github: keep the control hidden.
  });
});

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
  <template v-if="gh.configured">
    <DropdownMenu v-if="gh.user">
      <DropdownMenuTrigger as-child>
        <Button variant="ghost" size="sm" class="gap-1.5" data-testid="github-account">
          <img :src="gh.user.avatarUrl" alt="" class="size-5 rounded-full" />
          {{ gh.user.name || gh.user.login }}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem @select="signOut"><LogOut /> Sign out</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
    <Button v-else variant="outline" size="sm" data-testid="github-sign-in" @click="gh.signIn()">
      <LogIn /> Sign in with GitHub
    </Button>
    <DeviceCodeDialog />
  </template>
</template>
