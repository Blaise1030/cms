// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount } from "@vue/test-utils";
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
  updatedAt: new Date().toISOString(), headSha: "abc", checks: "passing", myState: "new", mine: false,
  description: "Rewrote the hero copy.", open: true, draft: false, canDecide: true, reason: null,
};
const DIFF = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,1 +1,1 @@\n-x\n+y\n";
const BINARY = "Binary files a/site/logo.png and b/site/logo.png differ\n";
const diffOf = (text: string, headSha: string | null = null) => ({ text, headSha });

async function mountChange(api: Record<string, unknown>) {
  h.daemon = createFakeDaemon({
    liveSiteUrl: vi.fn(async () => "https://acme.pages.dev"),
    reviewPreview: vi.fn(async () => ({ state: "none", headSha: "abc" })),
    ...api,
  });
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
  const w = mount(Host, { global: { plugins: [router] }, attachTo: document.body });
  // The store loads the diff parser lazily.
  await vi.waitFor(() => expect(w.text()).not.toContain("Loading change"));
  await flushPromises();
  return { w, router };
}

beforeEach(() => {
  localStorage.clear();
});
afterEach(() => vi.useRealTimers());

describe("ReviewChange", () => {
  it("shows the header and what changed", async () => {
    const { w } = await mountChange({
      review: vi.fn(async () => DETAIL),
      reviewDiff: vi.fn(async () => diffOf(DIFF)),
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

  it("fills the view with the preview by default and switches to what changed", async () => {
    const { w } = await mountChange({
      review: vi.fn(async () => DETAIL),
      reviewDiff: vi.fn(async () => diffOf(DIFF)),
    });
    const shown = (id: string) => (w.get(`[data-testid='${id}']`).element as HTMLElement).style.display !== "none";
    const tabs = w.findAll("[data-testid='review-view-tabs'] [role='tab']");
    expect(tabs.map((t) => t.text())).toEqual(["Preview", "What changed"]);
    expect(w.get("[data-testid='review-preview']").classes()).toContain("flex-1");
    expect(shown("review-preview")).toBe(true);
    expect(shown("review-changes")).toBe(false);
    await tabs[1]!.trigger("mousedown", { button: 0 });
    await flushPromises();
    expect(shown("review-preview")).toBe(false);
    expect(shown("review-changes")).toBe(true);
    // The decision stays in reach whichever view is showing.
    expect(w.find("[data-testid='review-greenlight']").isVisible()).toBe(true);
    expect(w.find("[data-testid='review-send-back']").isVisible()).toBe(true);
  });

  it("hides the live site link when there is no URL", async () => {
    const { w } = await mountChange({
      review: vi.fn(async () => DETAIL),
      reviewDiff: vi.fn(async () => diffOf(DIFF)),
      liveSiteUrl: vi.fn(async () => null),
    });
    expect(w.find("[data-testid='review-live-site']").exists()).toBe(false);
  });

  it("lists a binary-only change", async () => {
    const { w } = await mountChange({
      review: vi.fn(async () => DETAIL),
      reviewDiff: vi.fn(async () => diffOf(BINARY)),
    });
    expect(w.get("[data-testid='tree-stub']").text()).toContain("site/logo.png");
    expect(w.text()).toContain("Binary file changed.");
  });

  it("shows the gone copy with a way back on 404", async () => {
    const { w, router } = await mountChange({
      review: vi.fn(async () => {
        throw new ApiError("UNKNOWN", "This change is no longer waiting", 404);
      }),
      reviewDiff: vi.fn(async () => diffOf(DIFF)),
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
      reviewDiff: vi.fn(async () => diffOf(DIFF)),
    });
    expect(w.get("[role='alert']").text()).toContain("GitHub is unreachable");
    expect(w.text()).not.toContain("no longer waiting");
  });

  describe("preview", () => {
    const base = () => ({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => diffOf(DIFF)) });
    const READY = {
      state: "ready",
      headSha: "abc",
      url: "https://pr-7.acme.pages.dev/",
      liveUrl: "https://acme.pages.dev/",
    };
    const frame = (w: Awaited<ReturnType<typeof mountChange>>["w"], key: string) =>
      w.find(`iframe[data-testid='browser-frame-iframe-${key}']`);

    it("shows the change and the live site side by side once deployed", async () => {
      const reviewPreview = vi.fn(async () => READY);
      const { w } = await mountChange({ ...base(), reviewPreview });
      expect(reviewPreview).toHaveBeenCalledWith("site", 7);
      expect(frame(w, "change").attributes("src")).toBe("https://pr-7.acme.pages.dev/");
      expect(frame(w, "live").attributes("src")).toBe("https://acme.pages.dev/");
      // Another origin: its own scripts may run, never with orso's.
      expect(frame(w, "change").attributes("sandbox")).toBe("allow-scripts allow-same-origin allow-forms allow-popups");
      const tabs = w.findAll("[data-testid='review-compare'] [role='tab']");
      expect(tabs.map((t) => t.text())).toEqual(["This change", "Live site", "Side by side"]);
      expect(tabs[2]!.attributes("aria-selected")).toBe("true");
    });

    it("switches between the change, the live site and both", async () => {
      const { w } = await mountChange({ ...base(), reviewPreview: vi.fn(async () => READY) });
      const tab = (i: number) => w.findAll("[data-testid='review-compare'] [role='tab']")[i]!;
      await tab(0).trigger("mousedown", { button: 0 });
      await flushPromises();
      expect(frame(w, "change").exists()).toBe(true);
      expect(frame(w, "live").exists()).toBe(false);
      await tab(1).trigger("mousedown", { button: 0 });
      await flushPromises();
      expect(frame(w, "change").exists()).toBe(false);
      expect(frame(w, "live").attributes("src")).toBe("https://acme.pages.dev/");
      await tab(2).trigger("mousedown", { button: 0 });
      await flushPromises();
      expect(frame(w, "change").exists()).toBe(true);
      expect(frame(w, "live").exists()).toBe(true);
    });

    it("says the deploy is running and checks again every 10 s until it is ready", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const reviewPreview = vi
        .fn()
        .mockResolvedValueOnce({ state: "deploying", headSha: "abc", liveUrl: "https://acme.pages.dev/" })
        .mockResolvedValue(READY);
      const { w } = await mountChange({ ...base(), reviewPreview });
      expect(w.get("[data-testid='review-preview-deploying']").text()).toContain("Deploying preview…");
      // The live site is already there to look at, and the decision stays usable.
      expect(frame(w, "live").exists()).toBe(true);
      expect(w.get("[data-testid='review-greenlight']").attributes("disabled")).toBeUndefined();
      await vi.advanceTimersByTimeAsync(5000);
      expect(reviewPreview).toHaveBeenCalledTimes(1);
      await vi.advanceTimersByTimeAsync(5500);
      await flushPromises();
      expect(reviewPreview).toHaveBeenCalledTimes(2);
      expect(frame(w, "change").attributes("src")).toBe("https://pr-7.acme.pages.dev/");
      await vi.advanceTimersByTimeAsync(20_000);
      expect(reviewPreview).toHaveBeenCalledTimes(2);
    });

    it("waits for a hidden tab to come back before checking the deploy again", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const reviewPreview = vi
        .fn()
        .mockResolvedValueOnce({ state: "deploying", headSha: "abc" })
        .mockResolvedValue(READY);
      const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
      try {
        await mountChange({ ...base(), reviewPreview });
        await vi.advanceTimersByTimeAsync(30_000);
        expect(reviewPreview).toHaveBeenCalledTimes(1);
        visibility.mockReturnValue("visible");
        document.dispatchEvent(new Event("visibilitychange"));
        await flushPromises();
        expect(reviewPreview).toHaveBeenCalledTimes(2);
      } finally {
        visibility.mockRestore();
      }
    });

    it("links a failed deploy to the change's checks", async () => {
      const { w } = await mountChange({
        ...base(),
        reviewPreview: vi.fn(async () => ({ state: "failed", headSha: "abc", liveUrl: "https://acme.pages.dev/" })),
      });
      const failed = w.get("[data-testid='review-preview-failed']");
      expect(failed.text()).toContain("The preview deploy failed.");
      expect(w.get("[data-testid='review-preview-checks']").attributes("href")).toBe("https://example.test/7/checks");
    });

    it("explains a site without deploy previews and points at what changed", async () => {
      const { w } = await mountChange({
        ...base(),
        reviewPreview: vi.fn(async () => ({ state: "none", headSha: "abc" })),
        liveSiteUrl: vi.fn(async () => null),
      });
      expect(w.get("[data-testid='review-preview-none']").text()).toContain(
        "This site doesn't publish previews for changes.",
      );
      await w.get("[data-testid='review-preview-to-changes']").trigger("click");
      await flushPromises();
      expect((w.get("[data-testid='review-changes']").element as HTMLElement).style.display).not.toBe("none");
    });

    it("notes a missing live site on its own side", async () => {
      const { w } = await mountChange({
        ...base(),
        reviewPreview: vi.fn(async () => ({ state: "ready", headSha: "abc", url: "https://pr-7.acme.pages.dev/" })),
        liveSiteUrl: vi.fn(async () => null),
      });
      // Without a live site there is nothing to put beside the change.
      expect(frame(w, "change").exists()).toBe(true);
      const tabs = w.findAll("[data-testid='review-compare'] [role='tab']");
      await tabs[2]!.trigger("mousedown", { button: 0 });
      await flushPromises();
      expect(w.get("[data-testid='review-live-missing']").text()).toContain("No live site found");
    });

    it("shows a load error with Try again", async () => {
      const reviewPreview = vi
        .fn()
        .mockRejectedValueOnce(new ApiError("UNKNOWN", "GitHub is unreachable", 502))
        .mockResolvedValue(READY);
      const { w } = await mountChange({ ...base(), reviewPreview });
      expect(w.get("[data-testid='review-preview-error']").text()).toContain("GitHub is unreachable");
      await w.get("[data-testid='review-preview-retry']").trigger("click");
      await flushPromises();
      expect(frame(w, "change").exists()).toBe(true);
    });

    it("stops checking on unmount", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const reviewPreview = vi.fn(async () => ({ state: "deploying", headSha: "abc" }));
      const { w } = await mountChange({ ...base(), reviewPreview });
      w.unmount();
      await vi.advanceTimersByTimeAsync(30_000);
      expect(reviewPreview).toHaveBeenCalledTimes(1);
    });
  });

  describe("decision", () => {
    const base = () => ({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => diffOf(DIFF)) });

    it("offers Give greenlight and Send back", async () => {
      const { w } = await mountChange(base());
      const green = w.get("[data-testid='review-greenlight']");
      const back = w.get("[data-testid='review-send-back']");
      expect(green.text()).toBe("Give greenlight");
      expect(back.text()).toBe("Send back");
      expect(green.attributes("disabled")).toBeUndefined();
      expect(back.attributes("disabled")).toBeUndefined();
    });

    it("disables both and shows the reason when the reviewer cannot decide", async () => {
      const { w } = await mountChange({
        ...base(),
        review: vi.fn(async () => ({ ...DETAIL, canDecide: false, reason: "This is your change" })),
      });
      expect(w.get("[data-testid='review-greenlight']").attributes("disabled")).toBeDefined();
      expect(w.get("[data-testid='review-send-back']").attributes("disabled")).toBeDefined();
      expect(w.get("[data-testid='review-reason']").text()).toBe("This is your change");
    });

    it("greenlights with the head the reviewer saw and updates the row without reloading", async () => {
      const decideReview = vi.fn(async () => undefined);
      const review = vi.fn(async () => ({ ...DETAIL }));
      const { w } = await mountChange({ ...base(), review, decideReview });
      const { useReviewStore } = await import("./store");
      const store = useReviewStore();
      store.items = [{ ...DETAIL }];
      await w.get("[data-testid='review-greenlight']").trigger("click");
      await flushPromises();
      expect(decideReview).toHaveBeenCalledWith("site", 7, { decision: "greenlight", comment: undefined, headSha: "abc" });
      expect(store.items[0]?.myState).toBe("greenlit");
      expect(store.current?.myState).toBe("greenlit");
      expect(review).toHaveBeenCalledTimes(1);
    });

    it("says the change was updated and reloads detail and diff on head_moved", async () => {
      const decideReview = vi.fn(async () => {
        throw new ApiError("UNKNOWN", "This change was updated since you opened it", 409, { reason: "head_moved" });
      });
      const review = vi.fn(async () => ({ ...DETAIL }));
      const reviewDiff = vi.fn(async () => diffOf(DIFF));
      const { w } = await mountChange({ review, reviewDiff, decideReview });
      const { useReviewStore } = await import("./store");
      const store = useReviewStore();
      store.items = [{ ...DETAIL }];
      await w.get("[data-testid='review-greenlight']").trigger("click");
      await flushPromises();
      await vi.waitFor(() => expect(review).toHaveBeenCalledTimes(2));
      expect(reviewDiff).toHaveBeenCalledTimes(2);
      expect(w.text()).toContain("This change was updated since you opened it");
      expect(store.items[0]?.myState).toBe("new");
    });

    describe("send back dialog", () => {
      const dialog = () => new DOMWrapper(document.body);
      async function openDialog(api: Record<string, unknown>) {
        const m = await mountChange({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => diffOf(DIFF)), ...api });
        await m.w.get("[data-testid='review-send-back']").trigger("click");
        await flushPromises();
        await dialog().get("[data-testid='send-back-comment']").setValue("  Fix the hero  ");
        return m;
      }
      async function submit() {
        await dialog().get("[role='dialog'] form").trigger("submit");
        await flushPromises();
      }
      afterEach(() => {
        document.body.innerHTML = "";
      });

      it("sends the trimmed comment and closes on success", async () => {
        const decideReview = vi.fn(async () => undefined);
        await openDialog({ decideReview });
        await submit();
        expect(decideReview).toHaveBeenCalledWith("site", 7, { decision: "send_back", comment: "Fix the hero", headSha: "abc" });
        expect(dialog().find("[data-testid='send-back-comment']").exists()).toBe(false);
      });

      it("closes on head_moved and shows the message on the page", async () => {
        const decideReview = vi.fn(async () => {
          throw new ApiError("UNKNOWN", "This change was updated since you opened it", 409, { reason: "head_moved" });
        });
        const { w } = await openDialog({ decideReview });
        await submit();
        await vi.waitFor(() => expect(dialog().find("[data-testid='send-back-comment']").exists()).toBe(false));
        expect(w.get("[data-testid='review-decide-error']").text()).toContain("This change was updated since you opened it");
      });

      it("keeps the dialog open and shows another failure inside it", async () => {
        const decideReview = vi.fn(async () => {
          throw new ApiError("UNKNOWN", "This is your change", 403);
        });
        const { w } = await openDialog({ decideReview });
        await submit();
        expect(dialog().get("[data-testid='send-back-error']").text()).toContain("This is your change");
        expect((dialog().get("[data-testid='send-back-comment']").element as HTMLTextAreaElement).value).toBe("  Fix the hero  ");
        expect(w.find("[data-testid='review-decide-error']").exists()).toBe(false);
      });
    });
  });
});
