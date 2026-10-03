import { createRouter, createWebHistory, type RouteRecordRaw } from "vue-router";
import HomePage from "@/app/HomePage.vue";
import ProjectPage from "@/app/ProjectPage.vue";

// Project pages live under `/p/`, the prefix the daemon serves `index.html` for (see
// `SPA_ROUTE_PREFIXES` in src/static.rs), so deep links and reloads work.
export const routes: RouteRecordRaw[] = [
  { path: "/", name: "home", component: HomePage },
  { path: "/p/:id", name: "project", component: ProjectPage, props: true },
  { path: "/:pathMatch(.*)*", redirect: "/" },
];

export function projectPath(id: string) {
  return `/p/${encodeURIComponent(id)}`;
}

export function createAppRouter() {
  return createRouter({ history: createWebHistory(), routes });
}
