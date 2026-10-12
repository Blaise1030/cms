<script setup lang="ts">
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
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
    <Alert v-if="error" variant="destructive">
      <AlertDescription class="flex items-center justify-between gap-2">
        <span>{{ error }}</span>
        <Button size="sm" variant="outline" @click="$emit('retry')">Retry</Button>
      </AlertDescription>
    </Alert>
    <template v-else-if="loading && versions.length === 0">
      <Skeleton v-for="n in 3" :key="n" class="h-12 w-full" />
    </template>
    <p v-else-if="versions.length === 0" class="text-muted-foreground p-2 text-sm">No history yet</p>
    <button
      v-for="(v, i) in versions"
      v-else
      :key="v.sha"
      type="button"
      class="hover:bg-accent/60 aria-pressed:bg-accent focus-visible:ring-ring/50 flex w-full items-start gap-3 rounded-md px-2.5 py-2 text-left outline-none focus-visible:ring-3"
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
        <span class="block truncate text-sm font-medium">{{ versionTime(v.ts) }}</span>
        <span class="text-muted-foreground block truncate text-xs">{{ v.author }} · {{ v.summary }}</span>
      </span>
      <Badge
        variant="secondary"
        :class="{ 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-400': versionBadge(i, v) === 'Published' }"
        data-testid="history-badge"
      >
        {{ versionBadge(i, v) }}
      </Badge>
    </button>
  </div>
</template>
