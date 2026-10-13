import { createMemoryHistory, createRouter, type RouteRecordRaw } from "vue-router";
import { routes } from "@/router";

/** A router with the real Content route mounted at `/p/:id/content`, started at `start`. */
export async function contentRouter(start = "/p/p/content") {
  const content = routes.flatMap((r) => r.children ?? []).find((r) => r.name === "project-content")!;
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { ...content, path: "/p/:id/content/:collection?/:entry(.*)?" } as RouteRecordRaw,
      { path: "/other", component: { template: "<p>Other</p>" } },
    ],
  });
  await router.push(start);
  await router.isReady();
  return router;
}
