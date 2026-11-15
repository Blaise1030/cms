<script setup lang="ts">
import UAlert from "@nuxt/ui/components/Alert.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import USkeleton from "@nuxt/ui/components/Skeleton.vue";
import type { FileVersion } from "@/core/types";
import { versionBadge, versionTime } from "./history";

defineProps<{
  versions: FileVersion[];
  selectedSha: string | null;
  loading: boolean;
  error: string | null;
}>();
defineEmits<{ select: [sha: string]; retry: [] }>();
</script>

<template>
  <div class="flex flex-col gap-1 p-2" data-testid="history-list">
    <UAlert v-if="error" color="error" variant="subtle" orientation="horizontal" :description="error">
      <template #actions>
        <UButton color="neutral" variant="outline" size="sm" label="Retry" @click="$emit('retry')" />
      </template>
    </UAlert>
    <template v-else-if="loading && versions.length === 0">
      <USkeleton v-for="n in 3" :key="n" class="h-12 w-full" />
    </template>
    <p v-else-if="versions.length === 0" class="text-muted p-2 text-sm">No history yet</p>
    <UButton
      v-for="(v, i) in versions"
      v-else
      :key="v.sha"
      type="button"
      color="neutral"
      variant="ghost"
      class="aria-pressed:bg-elevated w-full items-start gap-3 px-2.5 py-2 text-left"
      :aria-pressed="v.sha === selectedSha"
      data-testid="history-version"
      @click="$emit('select', v.sha)"
    >
      <span
        aria-hidden="true"
        class="border-primary mt-1.5 size-2.5 shrink-0 rounded-full border-2"
        :class="{ 'bg-primary': v.sha === selectedSha || i === 0 }"
      />
      <span class="min-w-0 flex-1">
        <span class="text-highlighted block truncate text-sm font-medium">{{ versionTime(v.ts) }}</span>
        <span class="text-muted block truncate text-xs">{{ v.author }} · {{ v.summary }}</span>
      </span>
      <UBadge
        :color="versionBadge(i, v) === 'Published' ? 'success' : 'neutral'"
        variant="subtle"
        :label="versionBadge(i, v)"
        data-testid="history-badge"
      />
    </UButton>
  </div>
</template>
