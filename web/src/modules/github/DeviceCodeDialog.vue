<script setup lang="ts">
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useGithubStore } from "./store";

const gh = useGithubStore();

function copyCode() {
  if (gh.device) void navigator.clipboard?.writeText(gh.device.userCode).catch(() => {});
}
</script>

<template>
  <Dialog :open="!!gh.device" @update:open="(open) => !open && gh.cancel()">
    <DialogContent class="sm:max-w-sm">
      <DialogHeader>
        <DialogTitle>Sign in with GitHub</DialogTitle>
        <DialogDescription>
          Enter this code on GitHub to connect your account. This window closes once you approve.
        </DialogDescription>
      </DialogHeader>
      <div class="flex flex-col items-center gap-3 py-2">
        <code class="text-2xl font-semibold tracking-widest" data-testid="device-code">
          {{ gh.device?.userCode }}
        </code>
        <Button as="a" :href="gh.device?.verificationUri" target="_blank" rel="noopener" @click="copyCode">
          Copy code and open GitHub
        </Button>
        <p class="text-muted-foreground flex items-center gap-2 text-xs">
          <Spinner class="size-4" /> Waiting for approval…
        </p>
      </div>
    </DialogContent>
  </Dialog>
</template>
