<script setup lang="ts">
// App-level settings. A fixed sidebar holds the sections and their search; the
// content pane scrolls on its own. The open section lives in `?tab=`, so deep
// links and the command palette can point at one page.
import { computed, onBeforeUnmount, onMounted, ref, type Component } from "vue";
import { useRoute, useRouter } from "vue-router";
import { ArrowLeft, BotMessageSquare, GitBranch, Info, Package, Palette, Search } from "@lucide/vue";
import { InputGroup, InputGroupAddon, InputGroupInput, InputGroupText } from "@/components/ui/input-group";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
} from "@/components/ui/sidebar";
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
  icon: Component;
  component: Component;
}

const SECTIONS: SettingsSection[] = [
  {
    id: "assistant",
    label: "Assistant",
    title: "Assistant",
    description: "The assistant makes the changes you ask for. You can add another at any time.",
    icon: BotMessageSquare,
    component: AssistantSettings,
  },
  {
    id: "builder",
    label: "Site builder",
    title: "Site builder",
    description: "Lets orso build and preview your site.",
    icon: Package,
    component: SiteBuilderSettings,
  },
  {
    id: "github",
    label: "GitHub",
    title: "GitHub",
    description:
      "Git and the GitHub tools are optional for local work. Sign in to share previews and publish.",
    icon: GitBranch,
    component: GithubSettings,
  },
  {
    id: "appearance",
    label: "Appearance",
    title: "Appearance",
    description: "How orso looks and sounds while you work.",
    icon: Palette,
    component: AppearanceSettings,
  },
  {
    id: "about",
    label: "About",
    title: "About",
    description: "Version, your data, and where to find more.",
    icon: Info,
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

/** Same as the content collections sidebar (no primary tint — that belongs on the icon rail). */
const navItem = "h-7 rounded-lg px-2.5 text-[13px]";
const navActive = [
  "font-medium btn-raised-soft",
  "data-[active=true]:bg-background data-[active=true]:text-sidebar-accent-foreground",
  "data-[active=true]:hover:bg-background",
].join(" ");
</script>

<template>
  <SidebarProvider
    class="h-full min-h-0 overflow-hidden"
    :shortcut="false"
    data-testid="app-settings-page"
  >
    <Sidebar
      collapsible="none"
      class="bg-background h-full min-h-0 shrink-0"
      data-testid="app-settings-sidebar"
    >
      <SidebarContent
        data-testid="app-settings-nav"
        :class="[
          'gap-0 overflow-hidden px-3 py-2',
          macDesktop ? 'pt-11 fullscreen:pt-2' : '',
        ]"
      >
        <SidebarGroup class="shrink-0 py-1">
          <SidebarMenu class="gap-1 p-1.5">
            <SidebarMenuItem>
              <SidebarMenuButton
                type="button"
                :class="navItem"
                data-testid="app-settings-back"
                @click="goBack"
              >
                <ArrowLeft aria-hidden="true" />
                <span>Back to app</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroup>

        <SidebarGroup
          class="shrink-0 py-1"
          data-testid="app-settings-sidebar-header"
        >
          <SidebarGroupLabel class="text-sidebar-foreground/50 h-7 px-2 text-[11px] tracking-wide">
            Settings
          </SidebarGroupLabel>
          <SidebarGroupContent class="flex flex-col gap-2 px-1.5">
            <div ref="searchWrap">
              <InputGroup class="h-8 w-auto border-sidebar-border bg-card shadow-none">
                <InputGroupAddon align="inline-start">
                  <Search class="size-4 opacity-50" aria-hidden="true" />
                </InputGroupAddon>
                <InputGroupInput
                  v-model="query"
                  data-testid="app-settings-search"
                  placeholder="Find a setting"
                  aria-label="Find a setting"
                  class="h-8 text-[13px]"
                  @keydown.enter.prevent="selectFirstMatch"
                  @keydown.esc="query = ''"
                />
                <InputGroupAddon align="inline-end">
                  <InputGroupText>
                    <kbd
                      class="rounded border border-sidebar-border bg-sidebar-accent px-1.5 py-0.5 font-sans text-[10px] leading-none"
                      data-testid="app-settings-search-hint"
                    >
                      ⌘F
                    </kbd>
                  </InputGroupText>
                </InputGroupAddon>
              </InputGroup>
            </div>
          </SidebarGroupContent>
        </SidebarGroup>

        <template v-for="(group, index) in visibleGroups" :key="group.label">
          <div v-if="index > 0" class="h-2" aria-hidden="true" />
          <SidebarGroup class="py-1">
            <SidebarGroupLabel
              class="text-sidebar-foreground/50 h-7 px-2 text-[11px] tracking-wide"
            >
              {{ group.label }}
            </SidebarGroupLabel>
            <SidebarGroupContent>
              <!-- Padding keeps btn-raised-soft's shadow inside the scroller (overflow clips paint). -->
              <SidebarMenu class="gap-1 p-1.5">
                <SidebarMenuItem v-for="section in group.sections" :key="section.id">
                  <SidebarMenuButton
                    type="button"
                    :is-active="activeTab === section.id"
                    :class="[navItem, activeTab === section.id ? navActive : '']"
                    :aria-current="activeTab === section.id ? 'page' : undefined"
                    :data-testid="`app-settings-tab-${section.id}`"
                    @click="setTab(section.id)"
                  >
                    <component :is="section.icon" aria-hidden="true" />
                    <span>{{ section.label }}</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </template>

        <p
          v-if="!hasMatches"
          class="text-sidebar-foreground/50 px-3 py-2 text-xs"
          data-testid="app-settings-search-empty"
        >
          No settings found.
        </p>

        <div v-if="aboutMatches" class="mt-auto py-1">
          <SidebarMenu class="gap-1 p-1.5">
            <SidebarMenuItem>
              <SidebarMenuButton
                type="button"
                :is-active="activeTab === ABOUT.id"
                :class="[navItem, activeTab === ABOUT.id ? navActive : '']"
                :aria-current="activeTab === ABOUT.id ? 'page' : undefined"
                data-testid="app-settings-tab-about"
                @click="setTab(ABOUT.id)"
              >
                <component :is="ABOUT.icon" aria-hidden="true" />
                <span>{{ ABOUT.label }}</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </div>
      </SidebarContent>
    </Sidebar>

    <SidebarInset class="h-full min-h-0 w-auto overflow-y-auto">
      <div class="mx-auto flex w-full max-w-3xl flex-col gap-8 px-6 py-8 sm:px-8">
        <header class="flex items-start gap-3">
          <div
            class="bg-card text-foreground flex size-11 shrink-0 items-center justify-center rounded-xl border border-border/50 shadow-xs"
          >
            <component :is="activeSection.icon" class="size-5" aria-hidden="true" />
          </div>
          <div class="min-w-0 flex-1">
            <h2 class="text-2xl font-semibold tracking-tight">{{ activeSection.title }}</h2>
            <p class="text-muted-foreground mt-1 text-sm text-balance">
              {{ activeSection.description }}
            </p>
          </div>
          <div class="ml-auto shrink-0" data-testid="app-settings-account">
            <GithubAccount />
          </div>
        </header>

        <component :is="activeSection.component" />
      </div>
    </SidebarInset>
  </SidebarProvider>
</template>
