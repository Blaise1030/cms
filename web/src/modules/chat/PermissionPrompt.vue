<script setup lang="ts">
import {
  Confirmation,
  ConfirmationAction,
  ConfirmationActions,
  ConfirmationRequest,
  ConfirmationTitle,
} from "@/components/ai-elements/confirmation";
import type { PermissionView } from "./types";

defineProps<{ permission: PermissionView }>();
defineEmits<{ allow: []; deny: [] }>();
</script>

<template>
  <Confirmation
    :approval="{ id: permission.requestId }"
    state="approval-requested"
    class="border-amber-500/50"
    role="alertdialog"
    data-testid="permission-prompt"
  >
    <ConfirmationTitle>
      Agent wants to run {{ permission.tool }}
    </ConfirmationTitle>
    <ConfirmationRequest>
      <p v-if="permission.summary" class="text-muted-foreground text-xs">
        {{ permission.summary }}
      </p>
      <pre
        v-if="permission.command"
        class="bg-muted mt-2 overflow-x-auto rounded-md p-2 font-mono text-xs"
      >{{ permission.command }}</pre>
    </ConfirmationRequest>
    <ConfirmationActions>
      <ConfirmationAction
        size="sm"
        :disabled="permission.resolving"
        data-testid="permission-allow"
        @click="$emit('allow')"
      >
        Allow once
      </ConfirmationAction>
      <ConfirmationAction
        size="sm"
        variant="outline"
        :disabled="permission.resolving"
        data-testid="permission-deny"
        @click="$emit('deny')"
      >
        Deny
      </ConfirmationAction>
    </ConfirmationActions>
  </Confirmation>
</template>
