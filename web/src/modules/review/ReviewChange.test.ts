// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { defineComponent, h as el } from "vue";
import { TooltipProvider } from "reka-ui";
import { createMemoryHistory, createRouter } from "vue-router";
import { ApiError } from "@/core/api";
import type { ReviewDetail } from "@/core/types";
import { createFakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("../checkpoints/DiffView.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    __esModule: true,
    default: defineComponent({
      props: { diff: { type: Object, required: true } },
      setup: (p) => () => h("pre", { "data-testid": "diff-stub" }, p.diff.name),
    }),
  };
});
vi.mock("../checkpoints/ChangedFilesTree.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    __esModule: true,
    default: defineComponent({
      props: { files: { type: Array, required: true } },
      setup: (p) => () =>
        h("ul", { "data-testid": "tree-stub" }, (p.files as { path: string }[]).map((f) => h("li", f.path))),
    }),
  };
});

import ReviewChange from "./ReviewChange.vue";

const DETAIL: ReviewDetail = {
  number: 7, title: "Fresh homepage hero", author: "Ada", url: "https://example.test/7",
  updatedAt: new Date().toISOString(), headSha: "abc", checks: "passing", myState: "new",
  description: "Rewrote the hero copy.", open: true, draft: false, canDecide: true, reason: null,
};
const DIFF = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,1 +1,1 @@\n-x\n+y\n";
const BINARY = "Binary files a/site/logo.png and b/site/logo.png differ\n";

async function mountChange(api: Record<string, unknown>) {
  h.daemon = createFakeDaemon({ liveSiteUrl: vi.fn(async () => "https://acme.pages.dev"), ...api });
  setActivePinia(createPinia());
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/p/:id/review/:number?", name: "project-review", component: { render: () => null } }],
  });
  await router.push("/p/site/review/7");
  // Tooltips need the provider `UApp` gives the real app.
  const Host = defineComponent({
    setup: () => () =>
      el(TooltipProvider, null, { default: () => el(ReviewChange, { projectId: "site", number: 7 }) }),
  });
  const w = mount(Host, { global: { plugins: [router] } });
  // The store loads the diff parser lazily.
  await vi.waitFor(() => expect(w.text()).not.toContain("Loading change"));
  await flushPromises();
  return { w, router };
}

beforeEach(() => localStorage.clear());

describe("ReviewChange", () => {
  it("shows the header and what changed", async () => {
    const { w } = await mountChange({
      review: vi.fn(async () => DETAIL),
      reviewDiff: vi.fn(async () => DIFF),
    });
    expect(w.text()).toContain("Fresh homepage hero");
    expect(w.text()).toContain("Ada");
    expect(w.text()).toContain("Rewrote the hero copy.");
    const link = w.get("[data-testid='review-live-site']");
    expect(link.attributes("href")).toBe("https://acme.pages.dev");
    expect(link.text()).toContain("View live site");
    expect(w.text()).toContain("What changed");
    expect(w.get("[data-testid='tree-stub']").text()).toContain("a.txt");
    expect(w.get("[data-testid='diff-stub']").text()).toBe("a.txt");
  });

  it("hides the live site link when there is no URL", async () => {
    const { w } = await mountChange({
      review: vi.fn(async () => DETAIL),
      reviewDiff: vi.fn(async () => DIFF),
      liveSiteUrl: vi.fn(async () => null),
    });
    expect(w.find("[data-testid='review-live-site']").exists()).toBe(false);
  });

  it("lists a binary-only change", async () => {
    const { w } = await mountChange({
      review: vi.fn(async () => DETAIL),
      reviewDiff: vi.fn(async () => BINARY),
    });
    expect(w.get("[data-testid='tree-stub']").text()).toContain("site/logo.png");
    expect(w.text()).toContain("Binary file changed.");
  });

  it("shows the gone copy with a way back on 404", async () => {
    const { w, router } = await mountChange({
      review: vi.fn(async () => {
        throw new ApiError("UNKNOWN", "This change is no longer waiting", 404);
      }),
      reviewDiff: vi.fn(async () => DIFF),
    });
    expect(w.text()).toContain("This change is no longer waiting");
    await w.get("[data-testid='review-back']").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe("/p/site/review");
  });

  it("shows an error message when opening fails for another reason", async () => {
    const { w } = await mountChange({
      review: vi.fn(async () => {
        throw new ApiError("UNKNOWN", "GitHub is unreachable", 502);
      }),
      reviewDiff: vi.fn(async () => DIFF),
    });
    expect(w.get("[role='alert']").text()).toContain("GitHub is unreachable");
    expect(w.text()).not.toContain("no longer waiting");
  });
});
