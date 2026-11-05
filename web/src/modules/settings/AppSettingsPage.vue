<script setup lang="ts">
// App-level settings, separate from a project's settings. The open tab lives in
// `?tab=`, so deep links and the command palette can point at one section.
import { computed, type Component } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  ArrowLeft,
  BotMessageSquare,
  GitBranch,
  Package,
  Palette,
} from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import GithubAccount from "@/modules/github/GithubAccount.vue";
import { APP_SETTINGS_TABS, type AppSettingsTab } from "@/router";
import AppearanceSettings from "./AppearanceSettings.vue";
import AssistantSettings from "./AssistantSettings.vue";
import GithubSettings from "./GithubSettings.vue";
import SiteBuilderSettings from "./SiteBuilderSettings.vue";

const route = useRoute();
const router = useRouter();

const TABS: { id: AppSettingsTab; label: string; icon: Component }[] = [
  { id: "assistant", label: "Assistant", icon: BotMessageSquare },
  { id: "builder", label: "Site builder", icon: Package },
  { id: "github", label: "GitHub", icon: GitBranch },
  { id: "appearance", label: "Appearance", icon: Palette },
];

function isTab(value: unknown): value is AppSettingsTab {
  return typeof value === "string" && (APP_SETTINGS_TABS as readonly string[]).includes(value);
}

const activeTab = computed<AppSettingsTab>(() => {
  const raw = route.query.tab;
  const value = Array.isArray(raw) ? raw[0] : raw;
  return isTab(value) ? value : "assistant";
});

function setTab(value: unknown) {
  const tab = isTab(value) ? value : "assistant";
  if (tab === activeTab.value) return;
  void router.replace({ query: { ...route.query, tab } });
}

/** Back to the previous page when the browser has one; a deep link falls back home. */
function goBack() {
  const back = router.options.history.state.back;
  if (typeof back === "string" && back.length > 0) router.back();
  else void router.push("/");
}
</script>

<template>
  <div
    class="bg-background text-foreground flex h-full min-h-0 flex-col"
    data-testid="app-settings-page"
  >
    <header class="flex h-11 shrink-0 items-center gap-2 px-2">
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label="Back"
        data-testid="app-settings-back"
        @click="goBack"
      >
        <ArrowLeft />
      </Button>
      <div class="min-w-0 leading-tight">
        <h1 class="truncate text-sm font-semibold">Settings</h1>
        <p class="text-muted-foreground truncate text-xs">
          Assistant, site builder, GitHub, and appearance
        </p>
      </div>
      <div class="ml-auto flex items-center gap-2" data-testid="app-settings-account">
        <GithubAccount />
      </div>
    </header>

    <div class="min-h-0 flex-1 overflow-y-auto">
      <div class="mx-auto w-full max-w-4xl px-4 py-6 sm:px-6">
        <Tabs
          :model-value="activeTab"
          class="flex-col gap-6 md:flex-row md:items-start md:gap-8"
          @update:model-value="setTab"
        >
          <TabsList
            class="flex h-auto w-full max-w-full items-center justify-start gap-1 overflow-x-auto bg-transparent p-0 md:w-48 md:flex-col md:overflow-visible"
            aria-label="App settings"
          >
            <TabsTrigger
              v-for="tab in TABS"
              :key="tab.id"
              :value="tab.id"
              :data-testid="`app-settings-tab-${tab.id}`"
              class="h-9 flex-none justify-start gap-2 px-3 md:w-full"
            >
              <component :is="tab.icon" aria-hidden="true" />
              {{ tab.label }}
            </TabsTrigger>
          </TabsList>

          <div class="min-w-0 flex-1">
            <TabsContent value="assistant" class="m-0">
              <AssistantSettings />
            </TabsContent>
            <TabsContent value="builder" class="m-0">
              <SiteBuilderSettings />
            </TabsContent>
            <TabsContent value="github" class="m-0">
              <GithubSettings />
            </TabsContent>
            <TabsContent value="appearance" class="m-0">
              <AppearanceSettings />
            </TabsContent>
          </div>
        </Tabs>
      </div>
    </div>
  </div>
</template>
