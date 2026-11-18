// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";
import type { ReviewItem } from "@/core/types";
import { createFakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import ReviewInbox from "./ReviewInbox.vue";
import { useReviewStore } from "./store";

const item = (over: Partial<ReviewItem>): ReviewItem => ({
  number: 7,
  title: "Fresh homepage hero",
  author: "Ada",
  url: "https://example.test/7",
  updatedAt: new Date(Date.now() - 3 * 3600_000).toISOString(),
  headSha: "abc",
  checks: "passing",
  myState: "new",
  ...over,
});

async function mountInbox(reviews: ReviewItem[]) {
  const api = { reviews: vi.fn(async () => reviews) };
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/p/:id/review/:number?", name: "project-review", component: { render: () => null } },
    ],
  });
  await router.push("/p/site/review");
  const w = mount(ReviewInbox, { props: { projectId: "site" }, global: { plugins: [router] } });
  await flushPromises();
  return { w, router, api };
}

beforeEach(() => localStorage.clear());

describe("ReviewInbox", () => {
  it("renders title, author, relative time, checks and the state badge", async () => {
    const { w } = await mountInbox([
      item({ number: 1 }),
      item({ number: 2, myState: "sent_back", checks: "failing", title: "Second" }),
      item({ number: 3, myState: "greenlit", checks: "pending", title: "Third" }),
    ]);
    const rows = w.findAll("[data-testid='review-item']");
    expect(rows).toHaveLength(3);
    expect(rows[0]!.text()).toContain("Fresh homepage hero");
    expect(rows[0]!.text()).toContain("Ada");
    expect(rows[0]!.text()).toContain("3 hours ago");
    expect(rows[0]!.text()).toContain("New");
    expect(rows[0]!.find("[data-testid='review-checks']").attributes("data-checks")).toBe("passing");
    expect(rows[1]!.text()).toContain("You sent back");
    expect(rows[1]!.find("[data-testid='review-checks']").attributes("data-checks")).toBe("failing");
    expect(rows[2]!.text()).toContain("You greenlit");
  });

  it("shows the empty copy", async () => {
    const { w } = await mountInbox([]);
    expect(w.text()).toContain("Nothing waiting for your greenlight.");
  });

  it("opens a change by routing to project-review", async () => {
    const { w, router } = await mountInbox([item({ number: 7 })]);
    await w.get("[data-testid='review-item']").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.name).toBe("project-review");
    expect(router.currentRoute.value.params.number).toBe("7");
  });

  it("counts only new items as waiting", async () => {
    await mountInbox([item({ number: 1 }), item({ number: 2, myState: "greenlit" })]);
    expect(useReviewStore().waitingCount).toBe(1);
  });
});
