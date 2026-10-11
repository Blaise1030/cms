import { createRouter, createWebHistory, type RouteRecordRaw } from "vue-router";
import HomePage from "@/app/HomePage.vue";
import ProjectPage from "@/app/ProjectPage.vue";
import CheckpointsPanel from "@/modules/checkpoints/CheckpointsPanel.vue";
import ContentPanel from "@/modules/content/ContentPanel.vue";
import FilesPanel from "@/modules/files/FilesPanel.vue";

/** URL segment for a project workspace page (matches the tab labels). */
export type ProjectTab = "preview" | "content" | "code" | "history";

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
      { path: "content", name: "project-content", component: ContentPanel, props: (r) => ({projectId:r.params.id as string}) },
      {
        path: "code",
        name: "project-code",
        component: FilesPanel,
        props: (r) => ({ projectId: r.params.id as string }),
      },
      {
        path: "history",
        name: "project-history",
        component: CheckpointsPanel,
        props: (r) => ({ projectId: r.params.id as string }),
      },
    ],
  },
  { path: "/:pathMatch(.*)*", redirect: "/" },
];

export function projectPath(id: string, tab: ProjectTab = "preview") {
  return `/p/${encodeURIComponent(id)}/${tab}`;
}

export function projectTabFromRoute(name: string | symbol | null | undefined): ProjectTab {
  if (name === "project-content") return "content";
  if (name === "project-code") return "code";
  if (name === "project-history") return "history";
  return "preview";
}

export function createAppRouter() {
  return createRouter({ history: createWebHistory(), routes });
}
