// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";
import type { ReviewItem } from "@/core/types";
import { createFakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

// Menu items as plain buttons, so a test can pick one without a portal.
vi.mock("@nuxt/ui/components/DropdownMenu.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    __esModule: true,
    default: defineComponent({
      props: { items: { type: Array, required: true } },
      setup: (p, { slots }) => () =>
        h("div", [
          slots.default?.(),
          ...(p.items as Record<string, unknown>[]).map((item) =>
            h(
              "button",
              {
                "data-testid": item.testid,
                "data-checked": String(!!item.checked),
                onClick: () => (item.onUpdateChecked as () => void)(),
              },
              String(item.label),
            ),
          ),
        ]),
    }),
  };
});

import ReviewInbox from "./ReviewInbox.vue";
import { reviewKeys } from "./queries";
import { useProjectsStore } from "@/modules/projects/store";
import { queryPlugin } from "@/test/queryClient";

const item = (over: Partial<ReviewItem>): ReviewItem => ({
  number: 7,
  title: "Fresh homepage hero",
  author: "Ada",
  url: "https://example.test/7",
  updatedAt: new Date(Date.now() - 3 * 3600_000).toISOString(),
  headSha: "abc",
  checks: "passing",
  myState: "new",
  mine: false,
  ...over,
});

async function mountInbox(
  reviews: ReviewItem[],
  opts: { closed?: ReviewItem[]; more?: boolean; q?: string; labels?: string[]; repo?: string } = {},
) {
  const api = {
    reviews: vi.fn(async () => reviews),
    closedReviews: vi.fn(async () => ({ reviews: opts.closed ?? [], more: opts.more ?? false, labels: opts.labels })),
  };
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  if (opts.repo) useProjectsStore().projects = [{ id: "site", repo: opts.repo } as never];
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/p/:id/review/:number?", name: "project-review", component: { render: () => null } },
    ],
  });
  await router.push(opts.q === undefined ? "/p/site/review" : { path: "/p/site/review", query: { q: opts.q } });
  const { client, plugin } = queryPlugin();
  const w = mount(ReviewInbox, { props: { projectId: "site" }, global: { plugins: [router, plugin] } });
  await flushPromises();
  return { w, router, api, client };
}

beforeEach(() => localStorage.clear());
afterEach(() => vi.useRealTimers());

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

  it("titles the page and names the repository and labels it lists", async () => {
    const { w } = await mountInbox([], { repo: "acme/site", labels: ["orso", "content"] });
    expect(w.get("[data-testid='review-inbox-header'] h1").text()).toBe("Changes for review");
    expect(w.get("[data-testid='review-inbox-subtitle']").text()).toBe(
      "Shared changes on acme/site labelled orso, content",
    );
  });

  it("keeps the title and drops the subtitle parts it can't know", async () => {
    const bare = await mountInbox([]);
    expect(bare.w.get("[data-testid='review-inbox-header'] h1").text()).toBe("Changes for review");
    expect(bare.w.find("[data-testid='review-inbox-subtitle']").exists()).toBe(false);
    const labelsOnly = await mountInbox([], { labels: ["orso"] });
    expect(labelsOnly.w.get("[data-testid='review-inbox-subtitle']").text()).toBe("Shared changes labelled orso");
  });

  it("shows the empty copy", async () => {
    const { w } = await mountInbox([]);
    expect(w.text()).toContain("No changes waiting for review.");
  });

  it("opens a change by routing to project-review", async () => {
    const { w, router } = await mountInbox([item({ number: 7 })]);
    await w.get("[data-testid='review-item']").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.name).toBe("project-review");
    expect(router.currentRoute.value.params.number).toBe("7");
  });

  it("lists your own changes marked Yours, without counting them as waiting", async () => {
    const { w } = await mountInbox([item({ number: 1 }), item({ number: 2, mine: true, title: "Mine" })]);
    const rows = w.findAll("[data-testid='review-item']");
    expect(rows).toHaveLength(2);
    // Found by title: both rows are a few ms apart, so their newest-first order isn't fixed.
    const mine = rows.find((r) => r.text().includes("Mine"))!;
    const theirs = rows.find((r) => !r.text().includes("Mine"))!;
    expect(mine.find("[data-testid='review-mine']").text()).toBe("Yours");
    expect(mine.text()).not.toContain("New");
    expect(theirs.find("[data-testid='review-mine']").exists()).toBe(false);
  });

  it("reuses the inbox the rail already read instead of asking again", async () => {
    const api = { reviews: vi.fn(async () => [item({ number: 1 })]), closedReviews: vi.fn(async () => ({ reviews: [], more: false })) };
    h.daemon = createFakeDaemon(api);
    setActivePinia(createPinia());
    const { client, plugin } = queryPlugin();
    client.setQueryData(reviewKeys.inbox("site"), [item({ number: 1 })]);
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/p/:id/review/:number?", name: "project-review", component: { render: () => null } }],
    });
    await router.push("/p/site/review");
    const w = mount(ReviewInbox, { props: { projectId: "site" }, global: { plugins: [router, plugin] } });
    await flushPromises();
    expect(api.reviews).not.toHaveBeenCalled();
    expect(w.findAll("[data-testid='review-item']")).toHaveLength(1);
  });
});

describe("ReviewInbox like GitHub's list", () => {
  const rows = (w: Awaited<ReturnType<typeof mountInbox>>["w"]) =>
    w.findAll("[data-testid='review-item-title']").map((t) => t.text());
  const q = (router: Awaited<ReturnType<typeof mountInbox>>["router"]) => router.currentRoute.value.query.q;

  const ITEMS = [
    item({
      number: 83, title: "Landing page update", author: "bob", createdAt: new Date(Date.now() - 19 * 3600_000).toISOString(),
      labels: [{ name: "orso", color: "0e8a16" }, { name: "copy", color: "fbca04" }],
      checks: "failing", checksPassed: 8, checksTotal: 9, comments: 2, state: "open",
    }),
    item({ number: 5, title: "Pricing", author: "cy", myState: "greenlit", createdAt: new Date(Date.now() - 48 * 3600_000).toISOString(), comments: 0, checks: "none" }),
    item({ number: 9, title: "Blog post", author: "me", mine: true, createdAt: new Date(Date.now() - 1 * 3600_000).toISOString(), draft: true }),
  ];

  it("renders rows with label colours, checks passed/total, comments and the review tag", async () => {
    const { w } = await mountInbox(ITEMS);
    expect(w.get("[data-testid='review-list-card']").classes()).toEqual(
      expect.arrayContaining(["border-default", "bg-default", "rounded-lg", "border"]),
    );
    expect(w.find("input[type='checkbox']").exists()).toBe(false);
    // Newest first.
    expect(rows(w)).toEqual(["Blog post", "Landing page update", "Pricing"]);
    const row = w.findAll("[data-testid='review-item']")[1]!;
    const chips = row.findAll("[data-testid='review-item-label']");
    expect(chips.map((c) => c.text())).toEqual(["orso", "copy"]);
    expect((chips[0]!.element as HTMLElement).style.backgroundColor).toBe("#0e8a16");
    expect((chips[0]!.element as HTMLElement).style.color).toBe("#ffffff");
    expect((chips[1]!.element as HTMLElement).style.color).toBe("#1f2328");
    const meta = row.get("[data-testid='review-item-meta']").text();
    expect(meta).toContain("#83");
    expect(meta).toContain("bob opened 19 hours ago");
    expect(row.get("[data-testid='review-checks']").text()).toBe("8/9");
    expect(row.get("[data-testid='review-checks']").attributes("data-checks")).toBe("failing");
    expect(row.get("[data-testid='review-comments']").text()).toBe("2");
    expect(row.text()).toContain("New");
    const pricing = w.findAll("[data-testid='review-item']")[2]!;
    expect(pricing.find("[data-testid='review-comments']").exists()).toBe(false);
    expect(pricing.text()).toContain("You greenlit");
    const mine = w.findAll("[data-testid='review-item']")[0]!;
    expect(mine.get("[data-testid='review-mine']").text()).toBe("Yours");
    expect(mine.get("[data-testid='review-item-icon']").attributes("aria-label")).toBe("Draft");
  });

  it("searches by text and keeps the search in ?q=", async () => {
    const { w, router } = await mountInbox(ITEMS);
    await w.get("[data-testid='review-search']").setValue("is:open landing");
    await flushPromises();
    expect(q(router)).toBe("is:open landing");
    expect(rows(w)).toEqual(["Landing page update"]);
    w.unmount();
    // A reload with the query keeps it.
    const again = await mountInbox(ITEMS, { q: "#5" });
    expect(rows(again.w)).toEqual(["Pricing"]);
    expect((again.w.get("[data-testid='review-search']").element as HTMLInputElement).value).toBe("#5");
  });

  it("writes Author, Label, Reviews and Sort picks into the search, and clears on a second pick", async () => {
    const { w, router } = await mountInbox(ITEMS);
    await w.get("[data-testid='review-filter-author-bob']").trigger("click");
    await flushPromises();
    expect(q(router)).toBe("is:open author:bob");
    expect(rows(w)).toEqual(["Landing page update"]);
    await w.get("[data-testid='review-filter-author-bob']").trigger("click");
    await flushPromises();
    expect(q(router)).toBe("is:open");
    await w.get("[data-testid='review-filter-label-copy']").trigger("click");
    await flushPromises();
    expect(q(router)).toBe("is:open label:copy");
    expect(rows(w)).toEqual(["Landing page update"]);
    await w.get("[data-testid='review-filter-label-copy']").trigger("click");
    await flushPromises();
    await w.get("[data-testid='review-filter-review-yours']").trigger("click");
    await flushPromises();
    expect(q(router)).toBe("is:open review:yours");
    expect(rows(w)).toEqual(["Blog post"]);
    await w.get("[data-testid='review-filter-review-greenlit']").trigger("click");
    await flushPromises();
    expect(rows(w)).toEqual(["Pricing"]);
    await w.get("[data-testid='review-filter-review-greenlit']").trigger("click");
    await flushPromises();
    await w.get("[data-testid='review-sort-oldest']").trigger("click");
    await flushPromises();
    expect(q(router)).toBe("is:open sort:created-asc");
    expect(rows(w)).toEqual(["Pricing", "Landing page update", "Blog post"]);
    expect(w.get("[data-testid='review-menu-sort']").text()).toContain("Oldest");
    await w.get("[data-testid='review-sort-newest']").trigger("click");
    await flushPromises();
    expect(q(router)).toBe("is:open");
  });

  it("switches between Open and Closed with their counts", async () => {
    const closed = [
      item({ number: 70, title: "Old hero", state: "merged" }),
      item({ number: 71, title: "Dropped idea", state: "closed" }),
    ];
    const { w, router, api } = await mountInbox(ITEMS, { closed, more: true });
    expect(api.closedReviews).toHaveBeenCalledTimes(1);
    expect(w.get("[data-testid='review-open-count']").text()).toBe("3");
    expect(w.get("[data-testid='review-closed-count']").text()).toBe("2+");
    await w.get("[data-testid='review-show-closed']").trigger("click");
    await flushPromises();
    expect(q(router)).toBe("is:closed");
    expect(rows(w)).toEqual(["Old hero", "Dropped idea"]);
    const icons = w.findAll("[data-testid='review-item-icon']").map((i) => i.attributes("aria-label"));
    expect(icons).toEqual(["Merged", "Closed"]);
    // Closed changes carry no review tag.
    expect(w.findAll("[data-testid='review-item']")[0]!.text()).not.toContain("New");
    await w.get("[data-testid='review-show-open']").trigger("click");
    await flushPromises();
    expect(q(router)).toBe("is:open");
    expect(rows(w)).toHaveLength(3);
  });

  it("tells an empty search from an empty list", async () => {
    const { w, router } = await mountInbox(ITEMS, { q: "is:open nothing-like-this" });
    expect(w.get("[data-testid='review-empty-search']").text()).toContain("No changes match your search.");
    await w.get("[data-testid='review-clear-filters']").trigger("click");
    await flushPromises();
    expect(q(router)).toBe("is:open");
    expect(rows(w)).toHaveLength(3);
    w.unmount();
    const none = await mountInbox([], { q: "is:closed" });
    expect(none.w.get("[data-testid='review-empty']").text()).toContain("No closed changes.");
    expect(none.w.get("[data-testid='review-empty']").classes()).toContain("m-auto");
  });
});
