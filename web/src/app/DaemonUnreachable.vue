<script setup lang="ts">
import { ref } from "vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UCard from "@nuxt/ui/components/Card.vue";
import UFormField from "@nuxt/ui/components/FormField.vue";
import UInput from "@nuxt/ui/components/Input.vue";
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
    <UCard class="w-full max-w-md" role="alert" :ui="{ title: 'flex flex-col items-start gap-3' }">
      <template #title>
        <UBadge color="neutral" variant="subtle" icon="i-hugeicons-plug-01" class="uppercase">Offline</UBadge>
        {{ props.reason === "rejected" ? "Daemon refused the connection" : "Daemon unreachable" }}
      </template>
      <template #description>
        <template v-if="props.reason === 'rejected'">
          The daemon at <code class="text-highlighted">{{ props.origin }}</code> is running but
          rejected this app. Open the link printed by <code>orso serve</code> to refresh the
          token, or paste it below.
        </template>
        <template v-else>
          Nothing is answering at <code class="text-highlighted">{{ props.origin }}</code>. Start
          it with <code class="text-highlighted">orso serve</code>, then retry.
        </template>
      </template>
      <div class="flex flex-col gap-4">
        <UButton block :loading="props.connecting" :disabled="props.connecting" @click="emit('retry')">
          {{ props.connecting ? "Connecting…" : "Retry" }}
        </UButton>
        <form @submit.prevent="submit">
          <UFormField
            label="Daemon link (from the terminal)"
            :error="invalid ? 'Enter a valid http(s) URL.' : false"
            size="xs"
          >
            <div class="flex gap-2">
              <UInput
                id="daemon-link"
                v-model="link"
                class="flex-1"
                size="md"
                placeholder="http://127.0.0.1:7800/?t=…"
                :aria-invalid="invalid"
                autocomplete="off"
                spellcheck="false"
              />
              <UButton type="submit" color="neutral" variant="outline">Connect</UButton>
            </div>
          </UFormField>
        </form>
      </div>
    </UCard>
  </div>
</template>
