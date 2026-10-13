import { createRouter, createWebHistory, type RouteRecordRaw } from "vue-router";
import HomePage from "@/app/HomePage.vue";
import ProjectPage from "@/app/ProjectPage.vue";
import CheckpointsPanel from "@/modules/checkpoints/CheckpointsPanel.vue";
import ContentPanel from "@/modules/content/ContentPanel.vue";
import DesignPanel from "@/modules/design/DesignPanel.vue";
import FilesPanel from "@/modules/files/FilesPanel.vue";

/** URL segment for a project workspace page (matches the tab labels). */
export type ProjectTab = "preview" | "content" | "design" | "code" | "history";

const ProjectPreviewPage = { name: "ProjectPreviewPage", render: () => null };

// Project pages live under `/p/`, the prefix the daemon serves `index.html` for (see
// `SPA_ROUTE_PREFIXES` in src/static.rs), so deep links and reloads work.
export const routes: RouteRecordRaw[] = [
  { path: "/", name: "home", component: HomePage },
  {
    path: "/p/:id",
    component: ProjectPage,
    props: true,
    children: [
      { path: "", redirect: { name: "project-preview" } },
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
    ],
  },
  { path: "/:pathMatch(.*)*", redirect: "/" },
];

export function projectPath(id: string, tab: ProjectTab = "preview") {
  return `/p/${encodeURIComponent(id)}/${tab}`;
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

export function projectTabFromRoute(name: string | symbol | null | undefined): ProjectTab {
  if (name === "project-content") return "content";
  if (name === "project-design") return "design";
  if (name === "project-code") return "code";
  if (name === "project-history") return "history";
  return "preview";
}

export function createAppRouter() {
  return createRouter({ history: createWebHistory(), routes });
}
