import { createRouter, createWebHistory, type RouteRecordRaw } from "vue-router";
import HomePage from "@/app/HomePage.vue";
import ProjectPage from "@/app/ProjectPage.vue";
import { firstTab } from "@/lib/tabOrder";

// Each tab's panel loads when it is first opened, so start-up doesn't pay for the editor,
// the file trees and every other panel at once. The shell and the home page stay in the app chunk.
const CheckpointsPanel = () => import("@/modules/checkpoints/CheckpointsPanel.vue");
const ContentPanel = () => import("@/modules/content/ContentPanel.vue");
const DesignPanel = () => import("@/modules/design/DesignPanel.vue");
const FilesPanel = () => import("@/modules/files/FilesPanel.vue");
const ReviewPanel = () => import("@/modules/review/ReviewPanel.vue");
const AppSettingsPage = () => import("@/modules/settings/AppSettingsPage.vue");
const SettingsPanel = () => import("@/modules/settings/SettingsPanel.vue");

/** URL segment for a project workspace page (matches the tab labels). */
export type ProjectTab = "preview" | "content" | "design" | "code" | "history" | "review" | "settings";

/** App-level settings tabs, kept in the URL so a section can be linked to directly. */
export type AppSettingsTab = "assistant" | "builder" | "github" | "appearance" | "about";

export const APP_SETTINGS_TABS: readonly AppSettingsTab[] = [
  "assistant",
  "builder",
  "github",
  "appearance",
  "about",
];

const ProjectPreviewPage = { name: "ProjectPreviewPage", render: () => null };

// Project pages live under `/p/`, the prefix the daemon serves `index.html` for (see
// `SPA_ROUTE_PREFIXES` in src/static.rs), so deep links and reloads work.
export const routes: RouteRecordRaw[] = [
  { path: "/", name: "home", component: HomePage },
  { path: "/settings", name: "app-settings", component: AppSettingsPage },
  {
    path: "/p/:id",
    component: ProjectPage,
    props: true,
    children: [
      // A bare project link opens the first tab in the viewer's rail order.
      { path: "", redirect: () => ({ name: `project-${firstTab()}` }) },
      { path: "preview", name: "project-preview", component: ProjectPreviewPage },
      {
        path: "content/:collection?/:entry(.*)?",
        name: "project-content",
        component: ContentPanel,
        props: (r) => ({
          projectId: r.params.id as string,
          collection: (r.params.collection as string) || undefined,
          entry: (r.params.entry as string) || undefined,
        }),
      },
      {
        path: "design",
        name: "project-design",
        component: DesignPanel,
        props: (r) => ({ projectId: r.params.id as string }),
      },
      {
        path: "code/:path(.*)?",
        name: "project-code",
        component: FilesPanel,
        props: (r) => ({ projectId: r.params.id as string, path: (r.params.path as string) || undefined }),
      },
      {
        path: "history/:sha?",
        name: "project-history",
        component: CheckpointsPanel,
        props: (r) => ({ projectId: r.params.id as string, sha: (r.params.sha as string) || undefined }),
      },
      {
        path: "review/:number?",
        name: "project-review",
        component: ReviewPanel,
        props: (r) => ({
          projectId: r.params.id as string,
          number: r.params.number ? Number(r.params.number) : undefined,
          page: "conversation",
        }),
      },
      {
        path: "review/:number/files",
        name: "project-review-files",
        component: ReviewPanel,
        props: (r) => ({ projectId: r.params.id as string, number: Number(r.params.number), page: "files" }),
      },
      {
        path: "settings",
        name: "project-settings",
        component: SettingsPanel,
        props: (r) => ({ projectId: r.params.id as string }),
      },
    ],
  },
  { path: "/:pathMatch(.*)*", redirect: "/" },
];

/** A project tab's path; with no tab, the first one in the viewer's rail order. */
export function projectPath(id: string, tab: ProjectTab = firstTab()) {
  return `/p/${encodeURIComponent(id)}/${tab}`;
}

/** App-level settings, with the tab in the query so every section is linkable. */
export function settingsPath(tab: AppSettingsTab = "assistant") {
  return `/settings?tab=${tab}`;
}

/** Each segment is encoded but the slashes are kept, so nested paths read naturally in the URL. */
const segments = (path: string) => path.split("/").map(encodeURIComponent).join("/");

export function contentPath(id: string, collection?: string | null, entry?: string | null) {
  const base = projectPath(id, "content");
  if (!collection) return base;
  return entry ? `${base}/${encodeURIComponent(collection)}/${segments(entry)}` : `${base}/${encodeURIComponent(collection)}`;
}

export function codePath(id: string, path?: string | null) {
  const base = projectPath(id, "code");
  return path ? `${base}/${segments(path)}` : base;
}

export function historyPath(id: string, sha?: string | null) {
  const base = projectPath(id, "history");
  return sha ? `${base}/${encodeURIComponent(sha)}` : base;
}

/** A change's page: its Conversation (default) or its Review (the preview and what changed). */
export type ReviewPage = "conversation" | "files";

export function reviewPath(id: string, number?: number | null, page: ReviewPage = "conversation") {
  const base = projectPath(id, "review");
  if (!number) return base;
  return page === "files" ? `${base}/${number}/files` : `${base}/${number}`;
}

export function projectTabFromRoute(name: string | symbol | null | undefined): ProjectTab {
  if (name === "project-content") return "content";
  if (name === "project-design") return "design";
  if (name === "project-code") return "code";
  if (name === "project-history") return "history";
  if (name === "project-review" || name === "project-review-files") return "review";
  if (name === "project-settings") return "settings";
  return "preview";
}

export function createAppRouter() {
  return createRouter({ history: createWebHistory(), routes });
}
