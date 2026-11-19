<script setup lang="ts">
// App-level settings. A fixed sidebar holds the sections and their search; every
// section renders in one scrolling list, and a sidebar click scrolls to it. The
// last section jumped to lives in `?tab=`, so deep links and the command palette
// can still point at one section.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type Component } from "vue";
import { useRoute, useRouter } from "vue-router";
import UButton from "@nuxt/ui/components/Button.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UKbd from "@nuxt/ui/components/Kbd.vue";
import GithubAccount from "@/modules/github/GithubAccount.vue";
import ProjectRail from "@/app/ProjectRail.vue";
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
  /** Iconify name, e.g. `i-hugeicons-information-circle`. */
  icon: string;
  component: Component;
}

const SECTIONS: SettingsSection[] = [
  {
    id: "assistant",
    label: "Assistant",
    icon: "i-hugeicons-chat-bot",
    component: AssistantSettings,
  },
  {
    id: "builder",
    label: "Site builder",
    icon: "i-hugeicons-package",
    component: SiteBuilderSettings,
  },
  {
    id: "github",
    label: "GitHub",
    icon: "i-hugeicons-git-branch",
    component: GithubSettings,
  },
  {
    id: "appearance",
    label: "Appearance",
    icon: "i-hugeicons-paint-board",
    component: AppearanceSettings,
  },
  {
    id: "about",
    label: "About",
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

/** The section in view, kept in step with scrolling; starts at the linked one. */
const current = ref<AppSettingsTab>(activeTab.value);

const scroller = ref<HTMLElement | null>(null);
const sectionEls = new Map<AppSettingsTab, HTMLElement>();

function bindSection(id: AppSettingsTab, el: unknown) {
  if (el instanceof HTMLElement) sectionEls.set(id, el);
  else sectionEls.delete(id);
}

/** Ignore scroll-spy while a jump is animating, so the nav doesn't flicker through sections. */
let spyPausedUntil = 0;

function scrollToSection(id: AppSettingsTab, behavior: ScrollBehavior = "smooth") {
  const el = sectionEls.get(id);
  current.value = id;
  if (!el) return;
  spyPausedUntil = Date.now() + (behavior === "smooth" ? 700 : 0);
  el.scrollIntoView?.({ behavior, block: "start" });
}

function setTab(tab: AppSettingsTab) {
  scrollToSection(tab);
  if (tab !== activeTab.value) void router.replace({ query: { ...route.query, tab } });
}

function onScroll() {
  const root = scroller.value;
  if (!root || Date.now() < spyPausedUntil) return;
  if (root.scrollTop + root.clientHeight >= root.scrollHeight - 2) {
    current.value = SECTIONS[SECTIONS.length - 1]!.id;
    return;
  }
  const top = root.getBoundingClientRect().top + 48;
  let next = SECTIONS[0]!.id;
  for (const section of SECTIONS) {
    const el = sectionEls.get(section.id);
    if (el && el.getBoundingClientRect().top <= top) next = section.id;
  }
  current.value = next;
}

// A link to another section while the page is open (e.g. the command palette).
watch(activeTab, (tab) => {
  if (tab !== current.value) scrollToSection(tab);
});

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

onMounted(async () => {
  window.addEventListener("keydown", onKeydown);
  if (route.query.tab != null) {
    await nextTick();
    scrollToSection(activeTab.value, "instant");
  }
});
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));

/**
 * The project Settings was opened from (the page we would go back to), so its rail stays in
 * reach. None when opened from the projects list or a deep link.
 */
const fromProject: string | null = (() => {
  const back = router.options.history.state.back;
  if (typeof back !== "string" || back.length === 0) return null;
  const target = router.resolve(back);
  return target.matched.some((r) => r.path === "/p/:id") ? String(target.params.id) : null;
})();

/** Back to the previous page when the browser has one; a deep link falls back home. */
function goBack() {
  const back = router.options.history.state.back;
  if (typeof back === "string" && back.length > 0) router.back();
  else void router.push("/");
}

</script>

<template>
  <div class="flex h-full min-h-0 overflow-hidden" data-testid="app-settings-page">
    <ProjectRail v-if="fromProject" :project-id="fromProject" />
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
            :active="current === section.id"
            :aria-current="current === section.id ? 'page' : undefined"
            :data-active="current === section.id"
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
            :active="current === ABOUT.id"
            :aria-current="current === ABOUT.id ? 'page' : undefined"
            :data-active="current === ABOUT.id"
            data-testid="app-settings-tab-about"
            @click="setTab(ABOUT.id)"
          />
        </div>
      </nav>
    </aside>

    <main
      ref="scroller"
      class="bg-background h-full min-h-0 min-w-0 flex-1 overflow-y-auto"
      data-testid="app-settings-content"
      @scroll.passive="onScroll"
    >
      <div class="mx-auto flex w-full max-w-3xl flex-col gap-12 px-6 py-8 sm:px-8">
        <div class="flex items-center justify-between gap-3">
          <h1 class="text-highlighted text-2xl font-semibold tracking-tight">Settings</h1>
          <div class="shrink-0" data-testid="app-settings-account">
            <GithubAccount />
          </div>
        </div>

        <section
          v-for="section in SECTIONS"
          :id="`settings-${section.id}`"
          :key="section.id"
          :ref="(el) => bindSection(section.id, el)"
          class="flex scroll-mt-6 flex-col gap-6"
          :aria-label="section.label"
          :data-testid="`app-settings-section-${section.id}`"
        >
          <component :is="section.component" />
        </section>
      </div>
    </main>
  </div>
</template>
