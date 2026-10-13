import { computed, shallowRef, type Component } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  Code, File, FileText, Folder, FolderOpen, GitBranch, History, House, Keyboard, Monitor, MonitorPlay, Moon, Palette, Plus, RefreshCw, Sun, VolumeOff,
} from "@lucide/vue";
import { useDaemon } from "@/core/daemon";
import type { ContentEntrySummary } from "@/core/types";
import { THEME_LABELS, useTheme, type ThemePreference } from "@/lib/theme";
import { SOUND_PACKS, useTypingSound, type SoundPack, type TypingSoundSetting } from "@/lib/typingSound";
import { askAgentToGetLatest } from "@/modules/changes/agentRequests";
import { useFilesStore } from "@/modules/files/store";
import { useProjectsStore } from "@/modules/projects/store";
import { codePath, contentPath, projectPath, type ProjectTab } from "@/router";
import type { PaletteGroup, PaletteItem } from "./filter";

const TABS: { id: ProjectTab; label: string; icon: Component }[] = [
  { id: "preview", label: "Preview", icon: MonitorPlay },
  { id: "content", label: "Content", icon: FileText },
  { id: "design", label: "Design", icon: Palette },
  { id: "code", label: "Code", icon: Code },
  { id: "history", label: "History", icon: History },
];

const THEME_ICONS: Record<ThemePreference, Component> = { light: Sun, dark: Moon, system: Monitor };

interface ContentIndex { projectId: string; collections: string[]; entries: ContentEntrySummary[] }

function splitPath(path: string) {
  const at = path.lastIndexOf("/");
  return at < 0 ? { name: path, dir: undefined } : { name: path.slice(at + 1), dir: path.slice(0, at) };
}

/** The palette's command groups for the current route; pages the palette itself owns go through `ui`. */
export function usePaletteGroups(ui: { showChanges(): void; showNewChange(): void }) {
  const router = useRouter();
  const route = useRoute();
  const daemon = useDaemon();
  const projects = useProjectsStore();
  const files = useFilesStore();
  const theme = useTheme();
  const typingSound = useTypingSound();

  const projectId = computed(() => (typeof route.params.id === "string" ? route.params.id : null));
  const content = shallowRef<ContentIndex | null>(null);

  async function loadContent(id: string) {
    try {
      const collections = await daemon.api.listContentCollections(id);
      const lists = await Promise.all(
        collections.map((c) => daemon.api.listContentEntries(id, c.name).catch(() => [] as ContentEntrySummary[])),
      );
      if (projectId.value === id) content.value = { projectId: id, collections: collections.map((c) => c.name), entries: lists.flat() };
    } catch {
      // Not a content project, or the daemon is busy: the palette just lists no entries.
    }
  }

  /** Fetch what search-only groups list; called each time the palette opens. */
  function load() {
    const id = projectId.value;
    if (!id) return;
    void loadContent(id);
    if (files.projectId !== id || (!files.paths.length && !files.treeLoading)) void files.open(id);
  }

  const go = (path: string) => () => router.push(path);

  const groups = computed<PaletteGroup[]>(() => {
    const id = projectId.value;
    const out: PaletteGroup[] = [];

    const navigate: PaletteItem[] = [];
    if (id) {
      navigate.push({ id: "nav:home", label: "Go to Home", icon: House, run: go("/") });
      for (const tab of TABS) navigate.push({ id: `nav:${tab.id}`, label: `Go to ${tab.label}`, icon: tab.icon, run: go(projectPath(id, tab.id)) });
    }
    if (navigate.length) out.push({ heading: "Navigate", items: navigate });

    const others = projects.projects.filter((p) => p.id !== id);
    if (others.length) {
      out.push({
        heading: "Projects",
        items: others.map((p) => ({ id: `project:${p.id}`, label: `Open ${p.name}`, hint: p.repo ?? undefined, icon: FolderOpen, run: go(projectPath(p.id)) })),
      });
    }

    if (id) {
      out.push({
        heading: "Actions",
        items: [
          { id: "action:get-latest", label: "Sync from main", hint: "ask the agent to bring in the latest main", icon: RefreshCw, run: askAgentToGetLatest },
          { id: "action:switch-change", label: "Switch change…", hint: "branch", icon: GitBranch, stay: true, run: ui.showChanges },
          { id: "action:new-change", label: "New change…", hint: "branch", icon: Plus, run: ui.showNewChange },
        ],
      });

      const index = content.value?.projectId === id ? content.value : null;
      if (index) {
        out.push({
          heading: "Content",
          searchOnly: true,
          items: [
            ...index.collections.map((c) => ({ id: `collection:${c}`, label: c, hint: "collection", icon: Folder, run: go(contentPath(id, c)) })),
            ...index.entries.map((e) => ({ id: `entry:${e.collection}/${e.id}`, label: e.id, hint: e.collection, icon: FileText, run: go(contentPath(id, e.collection, e.id)) })),
          ],
        });
      }

      if (files.projectId === id && files.paths.length) {
        out.push({
          heading: "Files",
          searchOnly: true,
          items: files.paths.map((path) => {
            const { name, dir } = splitPath(path);
            return { id: `file:${path}`, label: name, hint: dir, icon: File, run: go(codePath(id, path)) };
          }),
        });
      }
    }

    out.push({
      heading: "Theme",
      items: (["light", "dark", "system"] as const).map((mode) => ({
        id: `theme:${mode}`,
        label: `${THEME_LABELS[mode]} theme`,
        icon: THEME_ICONS[mode],
        checked: theme.preference.value === mode,
        run: () => theme.setPreference(mode),
      })),
    });

    const soundOptions: { id: TypingSoundSetting; label: string }[] = [
      ...(Object.keys(SOUND_PACKS) as SoundPack[]).map((id) => ({ id, label: SOUND_PACKS[id].label })),
      { id: "off", label: "Off" },
    ];
    out.push({
      heading: "Typing sound",
      items: soundOptions.map(({ id, label }) => ({
        id: `typing-sound:${id}`,
        label: id === "off" ? "Typing sound off" : `${label} typing sound`,
        icon: id === "off" ? VolumeOff : Keyboard,
        checked: typingSound.setting.value === id,
        stay: true,
        run: () => typingSound.set(id),
      })),
    });

    return out;
  });

  return { projectId, groups, load };
}
