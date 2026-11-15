<script setup lang="ts">
import { defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate } from "vue-router";
import UAlert from "@nuxt/ui/components/Alert.vue";
import UBadge from "@nuxt/ui/components/Badge.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UEmpty from "@nuxt/ui/components/Empty.vue";
import UIcon from "@nuxt/ui/components/Icon.vue";
import NewChangeDialog from "@/modules/changes/NewChangeDialog.vue";
import type { Change } from "@/core/types";
import { askAgentToApplyDesign } from "./agentRequests";
import DesignTokensForm from "./DesignTokensForm.vue";
import { useDesignStore } from "./store";

// Tiptap and CodeMirror load with the first design, not with the app shell.
const MarkdownBodyEditor = defineAsyncComponent(() => import("@/modules/content/MarkdownBodyEditor.vue"));
const props = defineProps<{ projectId: string }>();
const store = useDesignStore();
const showNewChange = ref(false);

watch(() => props.projectId, id => void store.open(id), { immediate: true });
onBeforeRouteLeave(() => store.canLeave());
onBeforeRouteUpdate(() => store.canLeave());

async function save() { try { await store.save(); } catch { /* recorded on the store; the draft stays */ } }
async function resume(change: Change) { try { await store.resumeSave(change.branch); } catch { /* recorded on the store */ } }

function beforeUnload(event: BeforeUnloadEvent) { if (store.dirty) { event.preventDefault(); event.returnValue = ""; } }
onMounted(() => window.addEventListener("beforeunload", beforeUnload));
onBeforeUnmount(() => window.removeEventListener("beforeunload", beforeUnload));
</script>

<template>
  <div class="flex h-full min-h-0 flex-col" data-testid="design-panel">
    <UAlert
      v-if="store.invalid"
      role="alert"
      color="error"
      variant="subtle"
      icon="i-hugeicons-alert-circle"
      title="DESIGN.md has invalid frontmatter. Ask your assistant to fix it."
      class="m-4 w-auto"
      data-testid="design-invalid"
    >
      <template #description>
        <p class="text-muted text-xs">{{ store.invalid }}</p>
        <pre
          v-if="store.invalidSource"
          class="bg-muted text-default mt-2 max-h-96 overflow-auto rounded-md p-3 font-mono text-xs whitespace-pre-wrap"
          data-testid="design-invalid-source"
        >{{ store.invalidSource }}</pre>
      </template>
    </UAlert>
    <UEmpty
      v-else-if="store.doc && !store.doc.exists && !store.dirty"
      variant="naked"
      icon="i-hugeicons-paint-board"
      description="This project has no DESIGN.md yet."
      class="h-full pb-11"
    >
      <template #actions>
        <UButton type="button" label="Create design" data-testid="design-create" @click="store.create()" />
      </template>
    </UEmpty>
    <template v-else-if="store.doc">
      <header class="relative z-10 flex h-11 shrink-0 items-center gap-1 bg-background px-2">
        <div class="pointer-events-none absolute inset-x-0 top-full h-6 bg-linear-to-b from-background to-transparent" aria-hidden="true" />
        <h2 class="text-highlighted truncate px-1 text-sm font-medium">DESIGN.md</h2>
        <UBadge color="neutral" variant="subtle" size="sm" icon="i-hugeicons-git-branch" :label="store.doc.branch" class="min-w-0 truncate" />
        <div class="ml-auto flex shrink-0 items-center gap-2">
          <UButton
            type="button"
            color="neutral"
            variant="ghost"
            size="sm"
            label="Discard"
            data-testid="design-discard"
            :disabled="!store.dirty || store.saving"
            @click="store.discard()"
          />
          <UButton
            type="button"
            color="neutral"
            variant="outline"
            size="sm"
            :label="store.saving ? 'Saving…' : 'Save'"
            :loading="store.saving"
            data-testid="design-save"
            :disabled="!store.dirty || store.saving"
            @click="save"
          />
          <UButton
            type="button"
            size="sm"
            label="Apply to site"
            data-testid="design-apply"
            :disabled="store.dirty || !store.doc.exists"
            :title="store.dirty ? 'Save your changes first' : undefined"
            @click="askAgentToApplyDesign()"
          />
        </div>
      </header>
      <div class="min-h-0 flex-1 overflow-auto [container-type:size]">
        <div class="flex flex-col gap-3 px-4">
          <p v-if="store.error" role="alert" class="text-error text-sm" data-testid="design-error">{{ store.error }}</p>
          <UAlert
            v-if="store.needsChange"
            role="alert"
            color="warning"
            variant="subtle"
            icon="i-hugeicons-git-branch"
            description="The design cannot be edited on the live site. Create a change to save your edits."
            data-testid="design-needs-change"
          >
            <template #actions>
              <UButton type="button" size="sm" label="Create change" data-testid="design-create-change" @click="showNewChange = true" />
            </template>
          </UAlert>
          <UAlert
            v-if="store.changedOnDisk"
            role="status"
            color="neutral"
            variant="subtle"
            icon="i-hugeicons-information-circle"
            description="DESIGN.md changed on disk. Your unsaved draft is preserved."
            data-testid="design-changed-on-disk"
          >
            <template #actions>
              <UButton type="button" color="neutral" variant="outline" size="sm" label="Reload" data-testid="design-reload" @click="store.reloadFromDisk()" />
            </template>
          </UAlert>
          <UAlert
            v-if="store.conflict"
            role="alert"
            color="error"
            variant="subtle"
            icon="i-hugeicons-alert-circle"
            :description="`${store.conflict.message}. Your edits are kept.`"
            data-testid="design-conflict"
          >
            <template #actions>
              <UButton type="button" color="neutral" variant="outline" size="sm" label="Reload" data-testid="design-reload" @click="store.reloadFromDisk()" />
            </template>
          </UAlert>
        </div>
        <div class="@container flex min-h-full">
          <div class="flex min-h-full w-full flex-col @3xl:flex-row">
            <aside class="min-w-0 px-4 pt-6 pb-4 @3xl:sticky @3xl:top-0 @3xl:order-2 @3xl:max-h-[100cqh] @3xl:w-80 @3xl:shrink-0 @3xl:self-start @3xl:overflow-y-auto" data-testid="design-side">
              <DesignTokensForm :model-value="store.draftTokens" @update:model-value="store.setTokens" />
            </aside>
            <div class="min-w-0 flex-1 px-4 pt-4 pb-8 @3xl:order-1 @3xl:px-6">
              <div class="mx-auto w-full max-w-3xl">
                <MarkdownBodyEditor :body="store.draftBody" :original="store.doc.body" kind="markdown" :rich="true" @change="store.setBody($event.body)" />
              </div>
            </div>
          </div>
        </div>
      </div>
      <NewChangeDialog v-model:open="showNewChange" :project-id="props.projectId" @created="resume" />
    </template>
    <p v-else-if="store.loading" class="text-muted flex items-center gap-2 p-4 text-sm">
      <UIcon name="i-hugeicons-loading-03" class="size-4 animate-spin" /> Loading design…
    </p>
    <p v-else-if="store.error" role="alert" class="text-error p-4 text-sm" data-testid="design-error">{{ store.error }}</p>
  </div>
</template>
