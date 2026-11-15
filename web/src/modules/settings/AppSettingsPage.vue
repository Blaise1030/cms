<script setup lang="ts">
// App-level settings. A fixed sidebar holds the sections and their search; the
// content pane scrolls on its own. The open section lives in `?tab=`, so deep
// links and the command palette can point at one page.
import { computed, onBeforeUnmount, onMounted, ref, type Component } from "vue";
import { useRoute, useRouter } from "vue-router";
import UButton from "@nuxt/ui/components/Button.vue";
import UAvatar from "@nuxt/ui/components/Avatar.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UKbd from "@nuxt/ui/components/Kbd.vue";
import GithubAccount from "@/modules/github/GithubAccount.vue";
import { isMacDesktop } from "@/lib/desktop";
import { APP_SETTINGS_TABS, type AppSettingsTab } from "@/router";
import AboutSettings from "./AboutSettings.vue";
import AppearanceSettings from "./AppearanceSettings.vue";
import AssistantSettings from "./AssistantSettings.vue";
import GithubSettings from "./GithubSettings.vue";
import SiteBuilderSettings from "./SiteBuilderSettings.vue";

interface SettingsSection {
  id: AppSettingsTab;
  label: string;
  title: string;
  description: string;
  /** Iconify name, e.g. `i-hugeicons-information-circle`. */
  icon: string;
  component: Component;
}

const SECTIONS: SettingsSection[] = [
  {
    id: "assistant",
    label: "Assistant",
    title: "Assistant",
    description: "The assistant makes the changes you ask for. You can add another at any time.",
    icon: "i-hugeicons-chat-bot",
    component: AssistantSettings,
  },
  {
    id: "builder",
    label: "Site builder",
    title: "Site builder",
    description: "Lets orso build and preview your site.",
    icon: "i-hugeicons-package",
    component: SiteBuilderSettings,
  },
  {
    id: "github",
    label: "GitHub",
    title: "GitHub",
    description:
      "Git and the GitHub tools are optional for local work. Sign in to share previews and publish.",
    icon: "i-hugeicons-git-branch",
    component: GithubSettings,
  },
  {
    id: "appearance",
    label: "Appearance",
    title: "Appearance",
    description: "How orso looks and sounds while you work.",
    icon: "i-hugeicons-paint-board",
    component: AppearanceSettings,
  },
  {
    id: "about",
    label: "About",
    title: "About",
    description: "Version, your data, and where to find more.",
    icon: "i-hugeicons-information-circle",
    component: AboutSettings,
  },
];

const GROUPS: { label: string; ids: AppSettingsTab[] }[] = [
  { label: "Setup", ids: ["assistant", "builder", "github"] },
  { label: "Preferences", ids: ["appearance"] },
];

const ABOUT = SECTIONS.find((section) => section.id === "about")!;

const route = useRoute();
const router = useRouter();
const macDesktop = isMacDesktop();

const query = ref("");
const searchWrap = ref<HTMLElement | null>(null);

function isTab(value: unknown): value is AppSettingsTab {
  return typeof value === "string" && (APP_SETTINGS_TABS as readonly string[]).includes(value);
}

const activeTab = computed<AppSettingsTab>(() => {
  const raw = route.query.tab;
  const value = Array.isArray(raw) ? raw[0] : raw;
  return isTab(value) ? value : "assistant";
});

const activeSection = computed(
  () => SECTIONS.find((section) => section.id === activeTab.value) ?? SECTIONS[0]!,
);

function setTab(tab: AppSettingsTab) {
  if (tab === activeTab.value) return;
  void router.replace({ query: { ...route.query, tab } });
}

const needle = computed(() => query.value.trim().toLowerCase());

/** Only the nav labels are filtered; the card contents always show the whole section. */
function matches(section: SettingsSection) {
  return needle.value.length === 0 || section.label.toLowerCase().includes(needle.value);
}

const visibleGroups = computed(() =>
  GROUPS.map((group) => ({
    label: group.label,
    sections: group.ids
      .map((id) => SECTIONS.find((section) => section.id === id))
      .filter((section): section is SettingsSection => section != null)
      .filter(matches),
  })).filter((group) => group.sections.length > 0),
);

const aboutMatches = computed(() => matches(ABOUT));
const hasMatches = computed(() => visibleGroups.value.length > 0 || aboutMatches.value);

/** Enter jumps to the first section still visible under the current query. */
function selectFirstMatch() {
  const first = visibleGroups.value[0]?.sections[0] ?? (aboutMatches.value ? ABOUT : null);
  if (first) setTab(first.id);
}

function focusSearch() {
  const input = searchWrap.value?.querySelector("input");
  if (!input) return;
  input.focus();
  input.select();
}

function onKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
    event.preventDefault();
    focusSearch();
  }
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));

/** Back to the previous page when the browser has one; a deep link falls back home. */
function goBack() {
  const back = router.options.history.state.back;
  if (typeof back === "string" && back.length > 0) router.back();
  else void router.push("/");
}

</script>

<template>
  <div class="flex h-full min-h-0 overflow-hidden" data-testid="app-settings-page">
    <aside
      class="bg-background flex h-full min-h-0 w-64 shrink-0 flex-col"
      data-testid="app-settings-sidebar"
    >
      <nav
        aria-label="Settings"
        data-testid="app-settings-nav"
        :class="[
          'flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-3 py-3',
          macDesktop ? 'pt-11 fullscreen:pt-3' : '',
        ]"
      >
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          block
          class="justify-start"
          icon="i-hugeicons-arrow-left-01"
          label="Back to app"
          data-testid="app-settings-back"
          @click="goBack"
        />

        <div class="flex flex-col gap-2" data-testid="app-settings-sidebar-header">
          <p class="text-dimmed px-2 text-xs font-medium">Settings</p>
          <div ref="searchWrap">
            <UInput
              v-model="query"
              icon="i-hugeicons-search-01"
              size="sm"
              class="w-full"
              placeholder="Find a setting"
              aria-label="Find a setting"
              data-testid="app-settings-search"
              @keydown.enter.prevent="selectFirstMatch"
              @keydown.esc="query = ''"
            >
              <template #trailing>
                <UKbd value="⌘F" size="sm" data-testid="app-settings-search-hint" />
              </template>
            </UInput>
          </div>
        </div>

        <div v-for="group in visibleGroups" :key="group.label" class="flex flex-col gap-1">
          <p class="text-dimmed px-2 text-xs font-medium">{{ group.label }}</p>
          <UButton
            v-for="section in group.sections"
            :key="section.id"
            color="neutral"
            variant="ghost"
            active-variant="soft"
            size="sm"
            block
            class="justify-start"
            :icon="section.icon"
            :label="section.label"
            :active="activeTab === section.id"
            :aria-current="activeTab === section.id ? 'page' : undefined"
            :data-active="activeTab === section.id"
            :data-testid="`app-settings-tab-${section.id}`"
            @click="setTab(section.id)"
          />
        </div>

        <p v-if="!hasMatches" class="text-dimmed px-2 text-xs" data-testid="app-settings-search-empty">
          No settings found.
        </p>

        <div v-if="aboutMatches" class="mt-auto">
          <UButton
            color="neutral"
            variant="ghost"
            active-variant="soft"
            size="sm"
            block
            class="justify-start"
            :icon="ABOUT.icon"
            :label="ABOUT.label"
            :active="activeTab === ABOUT.id"
            :aria-current="activeTab === ABOUT.id ? 'page' : undefined"
            :data-active="activeTab === ABOUT.id"
            data-testid="app-settings-tab-about"
            @click="setTab(ABOUT.id)"
          />
        </div>
      </nav>
    </aside>

    <main class="bg-background h-full min-h-0 min-w-0 flex-1 overflow-y-auto">
      <div class="mx-auto flex w-full max-w-3xl flex-col gap-8 px-6 py-8 sm:px-8">
        <header class="flex items-start gap-3">
          <UAvatar
            :icon="activeSection.icon"
            color="neutral"
            size="2xl"
            :ui="{ root: 'rounded-lg bg-default ring ring-default', icon: 'text-highlighted size-5' }"
          />
          <div class="min-w-0 flex-1">
            <h2 class="text-highlighted text-2xl font-semibold tracking-tight">{{ activeSection.title }}</h2>
            <p class="text-muted mt-1 text-sm text-balance">
              {{ activeSection.description }}
            </p>
          </div>
          <div class="ml-auto shrink-0" data-testid="app-settings-account">
            <GithubAccount />
          </div>
        </header>

        <component :is="activeSection.component" />
      </div>
    </main>
  </div>
</template>
