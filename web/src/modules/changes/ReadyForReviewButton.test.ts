// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
import type { PrState, PrStatus } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

import { toast } from "vue-sonner";
import ReadyForReviewButton from "./ReadyForReviewButton.vue";
import { useChangesStore } from "./store";

const draft: PrStatus = {
  number: 4,
  url: "https://github.com/acme/site/pull/4",
  state: "draft",
  checks: "none",
  previewUrl: null,
};

function mountWith(status: PrStatus | null, api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  const store = useChangesStore();
  store.projectId = "site";
  store.active = { branch: "content/launch-week", title: "Launch week" };
  store.status = status;
  return { w: mount(ReadyForReviewButton), store };
}

const button = "[data-testid='ready-for-review']";
const link = "[data-testid='open-on-github']";

beforeEach(() => {
  vi.mocked(toast.error).mockClear();
});

describe("ReadyForReviewButton", () => {
  it("offers Ready for review only for a draft PR", () => {
    expect(mountWith(draft).w.find(button).exists()).toBe(true);
    expect(mountWith(null).w.find(button).exists()).toBe(false);
    for (const state of ["open", "approved", "merged", "closed"] as PrState[]) {
      expect(mountWith({ ...draft, state }).w.find(button).exists(), state).toBe(false);
    }
  });

  it("links the PR on GitHub once it is open or approved", () => {
    expect(mountWith(draft).w.find(link).exists()).toBe(false);
    expect(mountWith(null).w.find(link).exists()).toBe(false);
    for (const state of ["open", "approved"] as PrState[]) {
      const a = mountWith({ ...draft, state }).w.find(link);
      expect(a.text()).toBe("Open on GitHub");
      expect(a.attributes("href")).toBe(draft.url);
      expect(a.attributes("target")).toBe("_blank");
      expect(a.attributes("rel")).toContain("noopener");
    }
  });

  it("marks the PR ready and shows the fresh status", async () => {
    const markChangeReady = vi.fn(async () => ({ ...draft, state: "open" as const }));
    const changeStatus = vi.fn(async () => ({ ...draft, state: "open" as const }));
    const { w, store } = mountWith(draft, { markChangeReady, changeStatus });
    await w.find(button).trigger("click");
    await flushPromises();
    expect(markChangeReady).toHaveBeenCalledWith("site", "content/launch-week");
    expect(store.status?.state).toBe("open");
    expect(w.find(button).exists()).toBe(false);
    expect(w.find(link).exists()).toBe(true);
    store.stopStatusPolling();
  });

  it("surfaces a 403 and keeps the PR a draft", async () => {
    const markChangeReady = vi.fn(async () => {
      throw new ApiError("SANDBOX", "You don't have push access", 403);
    });
    const { w, store } = mountWith(draft, { markChangeReady });
    await w.find(button).trigger("click");
    await flushPromises();
    expect(toast.error).toHaveBeenCalledWith("You don't have push access");
    expect(store.status?.state).toBe("draft");
    expect(w.find(button).attributes("disabled")).toBeUndefined();
  });
});
