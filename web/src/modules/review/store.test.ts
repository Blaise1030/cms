// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import type { ReviewItem } from "@/core/types";
import { createFakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useReviewStore } from "./store";

const item = (number: number, myState: ReviewItem["myState"] = "new"): ReviewItem => ({
  number, title: `T${number}`, author: "Ada", url: "u", updatedAt: new Date().toISOString(),
  headSha: "s", checks: "none", myState,
});
const DETAIL = { ...item(7), description: null, open: true, draft: false, canDecide: true, reason: null };
const deferred = <T>() => {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
};

beforeEach(() => setActivePinia(createPinia()));

describe("review store", () => {
  it("keeps the open change visible when a later inbox load fails", async () => {
    const reviews = vi.fn().mockResolvedValueOnce([item(7)]).mockRejectedValueOnce(new Error("offline"));
    h.daemon = createFakeDaemon({ reviews, review: async () => DETAIL, reviewDiff: async () => ({ text: "", headSha: null }) });
    const store = useReviewStore();
    await store.load("p");
    await store.open("p", 7);
    await store.load("p");
    expect(store.inboxError).toBe("offline");
    expect(store.current?.number).toBe(7);
    expect(store.openError).toBeNull();
  });

  it("keeps an open failure when an inbox load succeeds", async () => {
    h.daemon = createFakeDaemon({
      reviews: async () => [item(7)],
      review: async () => { throw new Error("boom"); },
      reviewDiff: async () => ({ text: "", headSha: null }),
    });
    const store = useReviewStore();
    await store.open("p", 7);
    await store.load("p");
    expect(store.openError).toBe("boom");
    expect(store.gone).toBe(false);
  });

  it("drops the old project's state on switch and ignores stale loads", async () => {
    const a = deferred<ReviewItem[]>();
    const b = deferred<ReviewItem[]>();
    const reviews = vi.fn((id: string) => (id === "A" ? a.promise : b.promise));
    h.daemon = createFakeDaemon({ reviews });
    const store = useReviewStore();
    const la = store.load("A");
    const lb = store.load("B");
    b.resolve([item(2)]);
    await lb;
    a.resolve([item(1), item(3)]);
    await la;
    expect(store.items.map((i) => i.number)).toEqual([2]);
    expect(store.waitingCount).toBe(1);
    expect(store.loading).toBe(false);
  });

  it("clears items and the open change when the project changes", async () => {
    const pending = deferred<ReviewItem[]>();
    h.daemon = createFakeDaemon({
      reviews: vi.fn().mockResolvedValueOnce([item(1)]).mockReturnValueOnce(pending.promise),
      review: async () => DETAIL,
      reviewDiff: async () => ({ text: "", headSha: null }),
    });
    const store = useReviewStore();
    await store.load("A");
    await store.open("A", 7);
    void store.load("B");
    expect(store.items).toEqual([]);
    expect(store.current).toBeNull();
    expect(store.waitingCount).toBe(0);
    expect(store.loading).toBe(true);
  });

  it("reloads once when the diff's head differs from the detail's", async () => {
    const review = vi
      .fn()
      .mockResolvedValueOnce({ ...DETAIL, headSha: "old" })
      .mockResolvedValue({ ...DETAIL, headSha: "new" });
    const reviewDiff = vi.fn(async () => ({ text: "", headSha: "NEW" }));
    h.daemon = createFakeDaemon({ review, reviewDiff });
    const store = useReviewStore();
    await store.open("p", 7);
    expect(review).toHaveBeenCalledTimes(2);
    expect(reviewDiff).toHaveBeenCalledTimes(2);
    expect(store.current?.headSha).toBe("new");
    expect(store.opening).toBe(false);
  });

  it("reloads at most once when the heads keep disagreeing", async () => {
    const review = vi.fn(async () => ({ ...DETAIL, headSha: "a" }));
    const reviewDiff = vi.fn(async () => ({ text: "", headSha: "b" }));
    h.daemon = createFakeDaemon({ review, reviewDiff });
    const store = useReviewStore();
    await store.open("p", 7);
    expect(review).toHaveBeenCalledTimes(2);
    expect(store.current?.headSha).toBe("a");
    expect(store.opening).toBe(false);
  });
});
