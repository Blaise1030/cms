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

  describe("preview", () => {
    const base = () => ({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => DIFF) });

    it("prepares the current head and shows the building copy, polling every 2 s until ready", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const reviewPreview = vi
        .fn()
        .mockResolvedValueOnce({ state: "building" })
        .mockResolvedValue({ state: "ready", url: "/preview/site/reviews/7/abc/" });
      const prepareReviewPreview = vi.fn(async () => ({ state: "building" }));
      const { w } = await mountChange({ ...base(), prepareReviewPreview, reviewPreview });
      expect(prepareReviewPreview).toHaveBeenCalledWith("site", 7, "abc");
      expect(w.text()).toContain("Preparing the preview…");
      // The diff and decision stay usable while it builds.
      expect(w.find("[data-testid='diff-stub']").exists()).toBe(true);
      expect(w.get("[data-testid='review-greenlight']").attributes("disabled")).toBeUndefined();
      await vi.advanceTimersByTimeAsync(1900);
      expect(reviewPreview).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(200);
      expect(reviewPreview).toHaveBeenCalledWith("site", 7, "abc");
      await vi.advanceTimersByTimeAsync(2000);
      await flushPromises();
      const frame = w.get("iframe[data-testid='review-preview-frame']");
      expect(frame.attributes("src")).toBe("http://127.0.0.1:7800/preview/site/reviews/7/abc/");
      expect(frame.attributes("sandbox")).toBe("allow-same-origin");
      expect(w.text()).not.toContain("Preparing the preview…");
      const calls = reviewPreview.mock.calls.length;
      await vi.advanceTimersByTimeAsync(6000);
      expect(reviewPreview.mock.calls.length).toBe(calls);
    });

    it("shows the iframe at once when the first answer is ready", async () => {
      const { w } = await mountChange({
        ...base(),
        prepareReviewPreview: vi.fn(async () => ({ state: "ready", url: "/preview/site/reviews/7/abc/" })),
        reviewPreview: vi.fn(),
      });
      expect(w.find("iframe[data-testid='review-preview-frame']").exists()).toBe(true);
      expect(w.get("[data-testid='review-greenlight']").attributes("disabled")).toBeUndefined();
    });

    it("shows the failure and Try again prepares once more", async () => {
      const prepareReviewPreview = vi
        .fn()
        .mockResolvedValueOnce({ state: "failed", message: "npm ERR! build exploded" })
        .mockResolvedValueOnce({ state: "ready", url: "/preview/site/reviews/7/abc/" });
      const { w } = await mountChange({ ...base(), prepareReviewPreview, reviewPreview: vi.fn() });
      expect(w.get("[data-testid='review-preview-failed']").text()).toContain("npm ERR! build exploded");
      expect(w.find("[data-testid='diff-stub']").exists()).toBe(true);
      expect(w.get("[data-testid='review-greenlight']").attributes("disabled")).toBeUndefined();
      await w.get("[data-testid='review-preview-retry']").trigger("click");
      await flushPromises();
      expect(prepareReviewPreview).toHaveBeenCalledTimes(2);
      expect(w.find("iframe[data-testid='review-preview-frame']").exists()).toBe(true);
    });

    it("shows a failure when preparing itself fails", async () => {
      const { w } = await mountChange({
        ...base(),
        prepareReviewPreview: vi.fn(async () => {
          throw new ApiError("UNKNOWN", "GitHub is unreachable", 502);
        }),
        reviewPreview: vi.fn(),
      });
      expect(w.get("[data-testid='review-preview-failed']").text()).toContain("GitHub is unreachable");
    });

    it("prepares again when the poll finds the build gone, once", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const prepareReviewPreview = vi.fn(async () => ({ state: "building" }));
      const reviewPreview = vi
        .fn()
        .mockRejectedValueOnce(new ApiError("UNKNOWN", "not prepared", 404))
        .mockResolvedValue({ state: "ready", url: "/preview/site/reviews/7/abc/" });
      const { w } = await mountChange({ ...base(), prepareReviewPreview, reviewPreview });
      await vi.advanceTimersByTimeAsync(2100);
      await flushPromises();
      expect(prepareReviewPreview).toHaveBeenCalledTimes(2);
      expect(prepareReviewPreview).toHaveBeenLastCalledWith("site", 7, "abc");
      expect(w.text()).toContain("Preparing the preview…");
      await vi.advanceTimersByTimeAsync(2100);
      await flushPromises();
      expect(w.find("iframe[data-testid='review-preview-frame']").exists()).toBe(true);
    });

    it("fails when the build keeps vanishing", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const prepareReviewPreview = vi.fn(async () => ({ state: "building" }));
      const reviewPreview = vi.fn(async () => {
        throw new ApiError("UNKNOWN", "not prepared", 404);
      });
      const { w } = await mountChange({ ...base(), prepareReviewPreview, reviewPreview });
      await vi.advanceTimersByTimeAsync(2100);
      await vi.advanceTimersByTimeAsync(2100);
      await flushPromises();
      expect(prepareReviewPreview).toHaveBeenCalledTimes(2);
      expect(w.get("[data-testid='review-preview-failed']").text()).toContain("not prepared");
    });

    it("keeps polling after one network error", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const reviewPreview = vi
        .fn()
        .mockRejectedValueOnce(new Error("offline"))
        .mockResolvedValue({ state: "ready", url: "/preview/site/reviews/7/abc/" });
      const { w } = await mountChange({
        ...base(),
        prepareReviewPreview: vi.fn(async () => ({ state: "building" })),
        reviewPreview,
      });
      await vi.advanceTimersByTimeAsync(2100);
      await flushPromises();
      expect(w.find("[data-testid='review-preview-failed']").exists()).toBe(false);
      await vi.advanceTimersByTimeAsync(2100);
      await flushPromises();
      expect(w.find("iframe[data-testid='review-preview-frame']").exists()).toBe(true);
    });

    it("fails after three errors in a row", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const reviewPreview = vi.fn(async () => {
        throw new Error("offline");
      });
      const { w } = await mountChange({
        ...base(),
        prepareReviewPreview: vi.fn(async () => ({ state: "building" })),
        reviewPreview,
      });
      for (let i = 0; i < 3; i++) await vi.advanceTimersByTimeAsync(2100);
      await flushPromises();
      expect(reviewPreview).toHaveBeenCalledTimes(3);
      expect(w.get("[data-testid='review-preview-failed']").text()).toContain("offline");
    });

    it("reloads the change when preparing finds the head moved", async () => {
      const review = vi
        .fn()
        .mockResolvedValueOnce({ ...DETAIL })
        .mockResolvedValue({ ...DETAIL, headSha: "def" });
      const prepareReviewPreview = vi
        .fn()
        .mockRejectedValueOnce(new ApiError("CONFLICT", "This change was updated since you opened it", 409, { reason: "head_moved" }))
        .mockResolvedValue({ state: "building" });
      const { w } = await mountChange({
        review,
        reviewDiff: vi.fn(async () => DIFF),
        prepareReviewPreview,
        reviewPreview: vi.fn(async () => ({ state: "building" })),
      });
      await vi.waitFor(() => expect(prepareReviewPreview).toHaveBeenLastCalledWith("site", 7, "def"));
      expect(review).toHaveBeenCalledTimes(2);
      expect(w.find("[data-testid='review-preview-failed']").exists()).toBe(false);
    });

    it("stops polling on unmount", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const reviewPreview = vi.fn(async () => ({ state: "building" }));
      const { w } = await mountChange({
        ...base(),
        prepareReviewPreview: vi.fn(async () => ({ state: "building" })),
        reviewPreview,
      });
      await vi.advanceTimersByTimeAsync(2100);
      expect(reviewPreview).toHaveBeenCalledTimes(1);
      w.unmount();
      await vi.advanceTimersByTimeAsync(10000);
      expect(reviewPreview).toHaveBeenCalledTimes(1);
    });

    it("restarts for the new head after the change moved", async () => {
      const prepareReviewPreview = vi.fn(async () => ({ state: "building" }));
      const review = vi
        .fn()
        .mockResolvedValueOnce({ ...DETAIL })
        .mockResolvedValue({ ...DETAIL, headSha: "def" });
      const decideReview = vi.fn(async () => {
        throw new ApiError("CONFLICT", "This change was updated since you opened it", 409, { reason: "head_moved" });
      });
      const { w } = await mountChange({
        review,
        reviewDiff: vi.fn(async () => DIFF),
        decideReview,
        prepareReviewPreview,
        reviewPreview: vi.fn(async () => ({ state: "building" })),
      });
      expect(prepareReviewPreview).toHaveBeenLastCalledWith("site", 7, "abc");
      await w.get("[data-testid='review-greenlight']").trigger("click");
      await flushPromises();
      await vi.waitFor(() => expect(prepareReviewPreview).toHaveBeenLastCalledWith("site", 7, "def"));
    });
  });

  describe("decision", () => {
    const base = () => ({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => DIFF) });

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
      const reviewDiff = vi.fn(async () => DIFF);
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
        const m = await mountChange({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => DIFF), ...api });
        await m.w.get("[data-testid='review-send-back']").trigger("click");
        await flushPromises();
        await dialog().get("[data-testid='send-back-comment']").setValue("  Fix the hero  ");
        return m;
      }
      async function submit() {
        await dialog().get("form").trigger("submit");
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
