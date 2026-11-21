<script setup lang="ts">
// One assistant (or the site builder) in the setup list. The state machine and
// all copy come from `rows.ts`; this component only paints the row and reports
// the one action the user picked. It never talks to the daemon itself, so the
// same row works inside SetupCheck and inside a dialog.
import { computed } from "vue";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UCollapsible from "@nuxt/ui/components/Collapsible.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import UProgress from "@nuxt/ui/components/Progress.vue";
import { LABELS, type AssistantRow, type RowAction } from "./rows";
import { formatDownloadSize, sourceLabel } from "./presentation";

const props = defineProps<{
  /** A row built by `rows.ts` (`deriveRow` / `buildRows` / `siteBuilderRow`). */
  row: AssistantRow;
  /** The host is doing something with this row (opening chat); don't fire twice. */
  busy?: boolean;
  /** Hide the "Connect" button on a ready row where there is no chat to open (settings). */
  hideConnectedAction?: boolean;
  /** The assistant new chats start with (settings shows it, so the order's effect is visible). */
  isDefault?: boolean;
}>();

const emit = defineEmits<{
  action: [action: RowAction];
}>();

const downloading = computed(() => props.row.phase === "downloading");
/** A ready copy shows its version and where it came from; an update does too. */
const showVersion = computed(
  () => props.row.phase === "ready" || props.row.phase === "updateAvailable",
);
const size = computed(() =>
  props.row.phase === "ready" ? null : formatDownloadSize(props.row.provider.runtime?.downloadBytes),
);
const source = computed(() => (showVersion.value ? sourceLabel(props.row.source) : null));
const percent = computed(() => props.row.progress ?? 0);

type BadgeLook = { color: "success" | "warning" | "error"; variant: "subtle" | "outline" };

const statusBadge = computed<BadgeLook | null>(() => {
  if (!props.row.status || downloading.value || props.row.phase === "finishing") return null;
  switch (props.row.tone) {
    case "success":
      return { color: "success", variant: "subtle" };
    case "warning":
      return { color: "warning", variant: "outline" };
    case "destructive":
      return { color: "error", variant: "subtle" };
    default:
      return null;
  }
});

const showAction = computed(
  () =>
    props.row.action !== "none" && !(props.hideConnectedAction && props.row.action === "connect"),
);

/**
 * The plan's row machine labels the signed-out row "Connect" (it opens the
 * sign-in flow), while a connected assistant that can add more models keeps the
 * optional "Sign in" button. `rows.ts` stores the flow, so translate the label.
 */
const actionLabel = computed(() =>
  props.row.signInRequired && props.row.action === "signIn" ? LABELS.connect : props.row.actionLabel,
);

/** Cancel and retry are secondary; every other action is the row's primary step. */
const actionSecondary = computed(
  () => props.row.action === "cancel" || props.row.action === "retry",
);

const actionIcon = computed(() => {
  switch (props.row.action) {
    case "download":
      return "i-hugeicons-download-01";
    case "update":
      return "i-hugeicons-refresh";
    case "cancel":
      return "i-hugeicons-cancel-01";
    case "retry":
      return "i-hugeicons-arrow-turn-backward";
    case "signIn":
    case "connect":
      return "i-hugeicons-login-03";
    default:
      return undefined;
  }
});

const actionAria = computed(() => {
  switch (props.row.action) {
    case "download":
      return `Download ${props.row.name}`;
    case "cancel":
      return `Cancel downloading ${props.row.name}`;
    case "retry":
      return `Retry downloading ${props.row.name}`;
    case "update":
      return `Update ${props.row.name}`;
    case "connect":
      return `Connect to ${props.row.name}`;
    case "signIn":
      return props.row.signInRequired
        ? `Connect ${props.row.name}`
        : `Sign in to ${props.row.name} for more models`;
    default:
      return undefined;
  }
});
</script>

<template>
  <li
    class="flex flex-col gap-2 py-3"
    :data-testid="`assistant-row-${row.id}`"
    :data-phase="row.phase"
    :data-tone="row.tone"
    :data-action="row.action"
  >
    <div class="flex items-start gap-3">
      <!-- A host that can reorder rows puts its drag handle here (settings). -->
      <slot name="leading" />
      <div class="flex min-w-0 flex-1 flex-col gap-1">
        <div class="flex flex-wrap items-center gap-2">
          <span class="text-highlighted text-sm font-medium">{{ row.name }}</span>
          <UBadge
            v-if="row.recommended"
            color="neutral"
            variant="subtle"
            label="Recommended"
            data-testid="assistant-recommended"
          />
          <UBadge
            v-if="isDefault"
            color="primary"
            variant="subtle"
            label="Default"
            :data-testid="`assistant-default-${row.id}`"
          />
        </div>
        <p v-if="row.description" class="text-muted text-sm">{{ row.description }}</p>
        <p v-if="size" class="text-muted text-xs" :data-testid="`assistant-size-${row.id}`">
          {{ size }}
        </p>
        <p
          v-if="showVersion && row.version"
          class="text-muted text-xs"
          :data-testid="`assistant-version-${row.id}`"
        >
          Version {{ row.version }}<template v-if="source"> · {{ source }}</template>
        </p>
      </div>

      <div class="flex shrink-0 flex-wrap items-center justify-end gap-2">
        <UBadge
          v-if="statusBadge"
          :color="statusBadge.color"
          :variant="statusBadge.variant"
          :label="row.status"
          :data-testid="`assistant-status-${row.id}`"
        />
        <span
          v-else-if="row.phase === 'finishing'"
          class="text-muted inline-flex items-center gap-1.5 text-sm"
          :data-testid="`assistant-status-${row.id}`"
        >
          <UIcon name="i-hugeicons-loading-03" class="size-3.5 animate-spin" aria-hidden="true" />
          {{ row.status }}
        </span>

        <UButton
          v-if="showAction"
          size="sm"
          :color="actionSecondary ? 'neutral' : 'primary'"
          :variant="actionSecondary ? 'outline' : 'solid'"
          :icon="busy && row.action === 'connect' ? 'i-hugeicons-loading-03' : actionIcon"
          :ui="busy && row.action === 'connect' ? { leadingIcon: 'animate-spin' } : undefined"
          :label="actionLabel"
          :disabled="busy"
          :aria-label="actionAria"
          :data-testid="`assistant-action-${row.id}`"
          @click="emit('action', row.action)"
        />
      </div>
    </div>

    <div
      v-if="downloading"
      class="flex items-center gap-2"
      :data-testid="`assistant-progress-${row.id}`"
    >
      <UProgress
        :model-value="percent"
        size="sm"
        :get-value-label="() => `Downloading ${row.name}: ${row.status}`"
        class="max-w-48"
      />
      <span class="text-muted text-xs tabular-nums">{{ row.status }}</span>
    </div>

    <UAlert
      v-if="row.phase === 'downloadError'"
      color="error"
      variant="subtle"
      icon="i-hugeicons-alert-circle"
      :data-testid="`assistant-error-${row.id}`"
    >
      <template #description>
        <p>We couldn't download {{ row.name }}. Check your connection and try again.</p>
        <UCollapsible v-if="row.message" :unmount-on-hide="false" class="mt-1">
            <UButton
              variant="link"
              color="neutral"
              size="xs"
              class="px-0 underline"
              label="Details"
              data-testid="assistant-details-toggle"
            />
            <template #content>
              <p class="mt-1 text-xs break-words" data-testid="assistant-details">{{ row.message }}</p>
            </template>
          </UCollapsible>
      </template>
    </UAlert>
  </li>
</template>
