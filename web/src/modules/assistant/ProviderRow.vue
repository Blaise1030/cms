<script setup lang="ts">
// One assistant (or the site builder) in the setup list. The state machine and
// all copy come from `rows.ts`; this component only paints the row and reports
// the one action the user picked. It never talks to the daemon itself, so the
// same row works inside SetupCheck and inside a dialog.
import { computed } from "vue";
import {
  BotMessageSquare,
  CircleAlert,
  Download,
  LogIn,
  Package,
  RefreshCw,
  RotateCcw,
  X,
} from "@lucide/vue";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { LABELS, type AssistantRow, type RowAction } from "./rows";
import { formatDownloadSize, sourceLabel, toneBadge } from "./presentation";

const props = defineProps<{
  /** A row built by `rows.ts` (`deriveRow` / `buildRows` / `siteBuilderRow`). */
  row: AssistantRow;
  /** The host is doing something with this row (opening chat); don't fire twice. */
  busy?: boolean;
  /** Settings has no chat to open, so a ready row does not need a Connect button. */
  hideConnectedAction?: boolean;
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

const statusBadge = computed(() => {
  if (!props.row.status || downloading.value || props.row.phase === "finishing") return null;
  return toneBadge(props.row.tone);
});

/**
 * The plan's row machine labels the signed-out row "Connect" (it opens the
 * sign-in flow), while a connected assistant that can add more models keeps the
 * optional "Sign in" button. `rows.ts` stores the flow, so translate the label.
 */
const actionLabel = computed(() =>
  props.row.signInRequired && props.row.action === "signIn" ? LABELS.connect : props.row.actionLabel,
);

/** In Settings a connected row has nothing to connect to; keep the status badge. */
const showAction = computed(
  () =>
    props.row.action !== "none" &&
    !(props.hideConnectedAction && props.row.chatReady && props.row.action === "connect"),
);

const actionVariant = computed<"default" | "outline">(() =>
  props.row.action === "cancel" || props.row.action === "retry" ? "outline" : "default",
);

/** The row's leading mark: a package for the site builder, an assistant otherwise. */
const rowIcon = computed(() => (props.row.id === "node" ? Package : BotMessageSquare));

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
  <Item
    as="li"
    class="flex-col items-stretch gap-2 rounded-none border-0 bg-transparent p-0 py-3"
    :data-testid="`assistant-row-${row.id}`"
    :data-phase="row.phase"
    :data-tone="row.tone"
    :data-action="row.action"
  >
    <div class="flex items-start gap-3">
      <ItemMedia variant="icon" class="mt-0.5">
        <component :is="rowIcon" aria-hidden="true" />
      </ItemMedia>

      <ItemContent class="min-w-0 gap-1">
        <div class="flex flex-wrap items-center gap-2">
          <ItemTitle>{{ row.name }}</ItemTitle>
          <Badge v-if="row.recommended" variant="secondary" data-testid="assistant-recommended">
            Recommended
          </Badge>
        </div>
        <ItemDescription v-if="row.description">{{ row.description }}</ItemDescription>
        <p
          v-if="size"
          class="text-muted-foreground text-xs"
          :data-testid="`assistant-size-${row.id}`"
        >
          {{ size }}
        </p>
        <p
          v-if="showVersion && row.version"
          class="text-muted-foreground text-xs"
          :data-testid="`assistant-version-${row.id}`"
        >
          Version {{ row.version }}<template v-if="source"> · {{ source }}</template>
        </p>
      </ItemContent>

      <ItemActions class="shrink-0 flex-wrap justify-end">
        <Badge
          v-if="statusBadge"
          :variant="statusBadge.variant"
          :class="statusBadge.class"
          :data-testid="`assistant-status-${row.id}`"
        >
          {{ row.status }}
        </Badge>
        <span
          v-else-if="row.phase === 'finishing'"
          class="text-muted-foreground inline-flex items-center gap-1.5 text-sm"
          :data-testid="`assistant-status-${row.id}`"
        >
          <Spinner class="size-3.5" aria-hidden="true" /> {{ row.status }}
        </span>

        <Button
          v-if="showAction"
          size="sm"
          :variant="actionVariant"
          :disabled="busy"
          :aria-label="actionAria"
          :data-testid="`assistant-action-${row.id}`"
          @click="emit('action', row.action)"
        >
          <Spinner v-if="busy && row.action === 'connect'" class="size-3.5" aria-hidden="true" />
          <Download v-else-if="row.action === 'download'" aria-hidden="true" />
          <RefreshCw v-else-if="row.action === 'update'" aria-hidden="true" />
          <X v-else-if="row.action === 'cancel'" aria-hidden="true" />
          <RotateCcw v-else-if="row.action === 'retry'" aria-hidden="true" />
          <LogIn v-else-if="row.action === 'signIn' || row.action === 'connect'" aria-hidden="true" />
          {{ actionLabel }}
        </Button>
      </ItemActions>
    </div>

    <div
      v-if="downloading"
      class="flex items-center gap-2"
      :data-testid="`assistant-progress-${row.id}`"
    >
      <Progress
        :model-value="percent"
        :aria-label="`Downloading ${row.name}: ${row.status}`"
        class="max-w-48"
      />
      <span class="text-muted-foreground text-xs tabular-nums">{{ row.status }}</span>
    </div>

    <Alert
      v-if="row.phase === 'downloadError'"
      variant="destructive"
      :data-testid="`assistant-error-${row.id}`"
    >
      <CircleAlert aria-hidden="true" />
      <AlertDescription>
        <p>We couldn't download {{ row.name }}. Check your connection and try again.</p>
        <details v-if="row.message" class="mt-1">
          <summary class="cursor-pointer text-xs underline">Details</summary>
          <p class="mt-1 text-xs break-words">{{ row.message }}</p>
        </details>
      </AlertDescription>
    </Alert>
  </Item>
</template>
