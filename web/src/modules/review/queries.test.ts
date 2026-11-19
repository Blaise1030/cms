// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { createFakeDaemon } from "@/test/fakeDaemon";
import { queryPlugin } from "@/test/queryClient";

const hoisted = vi.hoisted(() => ({ daemon: null as unknown }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => hoisted.daemon }));

import { DEPLOY_POLL_MS, INBOX_POLL_MS, STALE_MS, useConversation, useInbox, usePreview } from "./queries";

/** Runs `setup` inside a component with its own QueryClient; returns what it gave back. */
function host<T>(setup: () => T) {
  let out!: T;
  const { client, plugin } = queryPlugin();
  const w = mount(defineComponent({ setup: () => ((out = setup()), () => h("div")) }), {
    global: { plugins: [plugin] },
  });
  return { out, w, client };
}

function setVisibility(state: "visible" | "hidden") {
  Object.defineProperty(document, "visibilityState", { configurable: true, get: () => state });
  document.dispatchEvent(new Event("visibilitychange", { bubbles: true }));
}

const row = (n: number, over: Record<string, unknown> = {}) => ({ number: n, myState: "new", mine: false, ...over });

afterEach(() => {
  vi.useRealTimers();
  setVisibility("visible");
});

describe("useInbox", () => {
  it("counts only other people's new changes as waiting", async () => {
    hoisted.daemon = createFakeDaemon({
      reviews: vi.fn(async () => [row(1), row(2, { myState: "greenlit" }), row(3, { mine: true })]),
    });
    const { out } = host(() => useInbox(() => "site"));
    await vi.waitFor(() => expect(out.items).toHaveLength(3));
    expect(out.waitingCount).toBe(1);
  });

  it("reads again every minute while visible, not while hidden, and on focus once stale", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const reviews = vi.fn(async () => [row(1)]);
    hoisted.daemon = createFakeDaemon({ reviews });
    const { w } = host(() => useInbox(() => "site"));
    await flushPromises();
    expect(reviews).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(INBOX_POLL_MS);
    expect(reviews).toHaveBeenCalledTimes(2);

    setVisibility("hidden");
    await vi.advanceTimersByTimeAsync(INBOX_POLL_MS * 3);
    expect(reviews).toHaveBeenCalledTimes(2);

    // Coming back with stale data reads it again.
    setVisibility("visible");
    window.dispatchEvent(new Event("focus"));
    await flushPromises();
    await vi.waitFor(() => expect(reviews).toHaveBeenCalledTimes(3));
    w.unmount();
  });

  it("does not read again on focus while the data is fresh", async () => {
    const reviews = vi.fn(async () => [row(1)]);
    hoisted.daemon = createFakeDaemon({ reviews });
    host(() => useInbox(() => "site"));
    await flushPromises();
    expect(STALE_MS).toBeGreaterThan(0);
    window.dispatchEvent(new Event("focus"));
    await flushPromises();
    expect(reviews).toHaveBeenCalledTimes(1);
  });
});

describe("usePreview", () => {
  it("checks a running deploy every 10 s and stops once it is ready", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const reviewPreview = vi
      .fn()
      .mockResolvedValueOnce({ state: "deploying", headSha: "abc" })
      .mockResolvedValue({ state: "ready", headSha: "abc", url: "https://pr.example" });
    hoisted.daemon = createFakeDaemon({ reviewPreview });
    const { out, w } = host(() => usePreview(() => "site", () => 7, () => "abc"));
    await flushPromises();
    expect(out.preview.value?.state).toBe("deploying");
    await vi.advanceTimersByTimeAsync(DEPLOY_POLL_MS);
    await vi.waitFor(() => expect(out.preview.value?.state).toBe("ready"));
    await vi.advanceTimersByTimeAsync(DEPLOY_POLL_MS * 3);
    expect(reviewPreview).toHaveBeenCalledTimes(2);
    w.unmount();
  });

  it("waits for a head before asking", async () => {
    const reviewPreview = vi.fn(async () => ({ state: "none", headSha: "" }));
    hoisted.daemon = createFakeDaemon({ reviewPreview });
    host(() => usePreview(() => "site", () => 7, () => undefined));
    await flushPromises();
    expect(reviewPreview).not.toHaveBeenCalled();
  });
});

describe("useConversation", () => {
  it("puts a posted comment in the conversation and marks the inbox stale", async () => {
    const item = { kind: "comment", id: 9, author: "me", at: "2026-01-02T00:00:00Z", body: "Hi", url: "u" };
    const reviewConversation = vi.fn(async () => ({ items: [], checks: [], reviewers: [] }));
    const commentOnReview = vi.fn(async () => item);
    hoisted.daemon = createFakeDaemon({ reviewConversation, commentOnReview });
    const { out, client } = host(() => useConversation(() => "site", () => 7, () => "abc"));
    await vi.waitFor(() => expect(out.conversation.value).not.toBeNull());
    client.setQueryData(["review", "site", "inbox"], []);
    expect(await out.comment("Hi")).toBe(true);
    expect(commentOnReview).toHaveBeenCalledWith("site", 7, "Hi");
    expect(out.conversation.value?.items.some((i) => i.kind === "comment" && i.body === "Hi")).toBe(true);
    expect(client.getQueryState(["review", "site", "inbox"])?.isInvalidated).toBe(true);
  });

  it("refuses a blank comment without asking GitHub", async () => {
    const commentOnReview = vi.fn();
    hoisted.daemon = createFakeDaemon({
      reviewConversation: vi.fn(async () => ({ items: [], checks: [], reviewers: [] })),
      commentOnReview,
    });
    const { out } = host(() => useConversation(() => "site", () => 7, () => "abc"));
    expect(await out.comment("   ")).toBe(false);
    expect(commentOnReview).not.toHaveBeenCalled();
  });
});
