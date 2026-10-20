<script setup lang="ts">
import { ref } from "vue";
import { PlugZap } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import type { UnreachableReason } from "@/core/daemon";

const props = defineProps<{
  origin: string;
  reason: UnreachableReason | null;
  connecting?: boolean;
}>();

const emit = defineEmits<{
  retry: [];
  connect: [link: string];
}>();

const link = ref("");
const invalid = ref(false);

function submit() {
  const value = link.value.trim();
  if (!value) return;
  try {
    const url = new URL(value);
    invalid.value = url.protocol !== "http:" && url.protocol !== "https:";
  } catch {
    invalid.value = true;
  }
  if (!invalid.value) emit("connect", value);
}
</script>

<template>
  <div class="flex h-full min-h-screen items-center justify-center p-6">
    <Card class="w-full max-w-md" role="alert">
      <CardHeader>
        <div class="text-muted-foreground mb-2 flex items-center gap-2">
          <PlugZap class="size-5" />
          <span class="text-xs tracking-wide uppercase">Offline</span>
        </div>
        <CardTitle>
          {{ props.reason === "rejected" ? "Daemon refused the connection" : "Daemon unreachable" }}
        </CardTitle>
        <CardDescription>
          <template v-if="props.reason === 'rejected'">
            The daemon at <code class="text-foreground">{{ props.origin }}</code> is running but
            rejected this app. Open the link printed by <code>orso serve</code> to refresh the
            token, or paste it below.
          </template>
          <template v-else>
            Nothing is answering at <code class="text-foreground">{{ props.origin }}</code>. Start
            it with <code class="text-foreground">orso serve</code>, then retry.
          </template>
        </CardDescription>
      </CardHeader>
      <CardContent class="flex flex-col gap-4">
        <Button :disabled="props.connecting" @click="emit('retry')">
          {{ props.connecting ? "Connecting…" : "Retry" }}
        </Button>
        <form class="flex flex-col gap-2" @submit.prevent="submit">
          <label for="daemon-link" class="text-muted-foreground text-xs">
            Daemon link (from the terminal)
          </label>
          <div class="flex gap-2">
            <Input
              id="daemon-link"
              v-model="link"
              placeholder="http://127.0.0.1:7800/?t=…"
              :aria-invalid="invalid"
              autocomplete="off"
              spellcheck="false"
            />
            <Button type="submit" variant="secondary">Connect</Button>
          </div>
          <p v-if="invalid" class="text-destructive text-xs">Enter a valid http(s) URL.</p>
        </form>
      </CardContent>
    </Card>
  </div>
</template>
