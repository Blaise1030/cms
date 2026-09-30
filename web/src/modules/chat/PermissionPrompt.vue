<script setup lang="ts">
import { ShieldQuestion } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { PermissionView } from "./types";

defineProps<{ permission: PermissionView }>();
defineEmits<{ allow: []; deny: [] }>();
</script>

<template>
  <Card class="border-amber-500/50 py-3" role="alertdialog" data-testid="permission-prompt">
    <CardHeader class="px-4">
      <CardTitle class="flex items-center gap-2 text-sm">
        <ShieldQuestion class="size-4 text-amber-500" />
        Agent wants to run {{ permission.tool }}
      </CardTitle>
    </CardHeader>
    <CardContent class="grid gap-3 px-4">
      <p v-if="permission.summary" class="text-muted-foreground text-xs">{{ permission.summary }}</p>
      <pre
        v-if="permission.command"
        class="bg-muted overflow-x-auto rounded-md p-2 font-mono text-xs"
      >{{ permission.command }}</pre>
      <div class="flex gap-2">
        <Button size="sm" :disabled="permission.resolving" data-testid="permission-allow" @click="$emit('allow')">
          Allow once
        </Button>
        <Button
          size="sm"
          variant="outline"
          :disabled="permission.resolving"
          data-testid="permission-deny"
          @click="$emit('deny')"
        >
          Deny
        </Button>
      </div>
    </CardContent>
  </Card>
</template>
