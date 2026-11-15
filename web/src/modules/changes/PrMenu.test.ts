// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { STATUS_POLL_MS } from "./store";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
import type { PrStatus } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("@/lib/toast", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

import { toast } from "@/lib/toast";
import PrMenu from "./PrMenu.vue";
import { useChangesStore } from "./store";

const draft: PrStatus = {
  number: 4,
  url: "https://github.com/acme/site/pull/4",
  state: "draft",
  checks: "failing",
  previewUrl: null,
  mergeable: false,
  mergeableState: "dirty",
  upToDate: true,
};

let mounted: VueWrapper | null = null;

/** Open the real menu from the keyboard; its content is portalled into `document.body`. */
async function openMenu() {
  await new DOMWrapper(document.body).get("[data-testid='pr-menu']").trigger("keydown", { key: "Enter" });
  await flushPromises();
}

async function mountWith(status: PrStatus | null, api: Record<string, unknown> = {}) {
  mounted?.unmount();
  document.body.innerHTML = "";
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  const store = useChangesStore();
  store.projectId = "site";
  store.active = { branch: "content/launch-week", title: "Launch week" };
  store.prUrl = draft.url;
  store.status = status;
  mounted = mount(PrMenu, { props: { variant: "secondary" }, attachTo: document.body });
  await openMenu();
  return { w: new DOMWrapper(document.body), store };
}

const ready = "[data-testid='ready-for-review']";

beforeEach(() => {
  vi.mocked(toast.error).mockClear();
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
  document.body.innerHTML = "";
});

describe("PrMenu", () => {
  it("shows the PR number and status", async () => {
    const { w } = await mountWith(draft);
    expect(w.find("[data-testid='pr-menu']").text()).toContain("Review");
    expect(w.find("[data-testid='pr-status']").text()).toBe("Draft · Checks failing");
  });

  it("shows a green check for a merged PR instead of its failing checks", async () => {
    const { w } = await mountWith({ ...draft, state: "merged", checks: "failing" });
    expect(w.find("[data-testid='pr-status']").text()).toBe("Published");
    const status = w.find("[data-testid='pr-status-icon']");
    expect(status.exists()).toBe(true);
    expect(status.classes()).toContain("text-success");
    expect(status.classes()).not.toContain("text-error");
    expect(w.find("[data-testid='pr-trigger-icon']").classes()).toContain("text-success");
  });

  it("shows a red cross for an open PR with failing checks", async () => {
    const { w } = await mountWith({ ...draft, state: "open", checks: "failing" });
    expect(w.find("[data-testid='pr-status']").text()).toBe("Waiting for greenlight · Checks failing");
    expect(w.find("[data-testid='pr-status-icon']").classes()).toContain("text-error");
  });

  it("links the PR before its status has loaded", async () => {
    const { w } = await mountWith(null);
    expect(w.find("[data-testid='pr-menu']").text()).toContain("Review");
    expect(w.find("[data-testid='pr-status']").exists()).toBe(false);
    expect(w.find("[data-testid='pr-link']").attributes("href")).toBe(draft.url);
  });

  it("links the deployed preview, never a non-http one", async () => {
    const { w } = await mountWith({ ...draft, previewUrl: "https://lw.pages.dev" });
    const a = w.find("[data-testid='preview-deploy-link']");
    expect(a.attributes("href")).toBe("https://lw.pages.dev");
    expect(a.attributes("rel")).toContain("noopener");
    for (const previewUrl of ["javascript:alert(1)", "//evil.dev"]) {
      expect((await mountWith({ ...draft, previewUrl })).w.find("[data-testid='preview-deploy-link']").exists()).toBe(false);
    }
  });

  it("opens menu links in the browser instead of navigating the app", async () => {
    const open = vi.fn();
    const original = window.open;
    window.open = open;
    try {
      const { w } = await mountWith({ ...draft, previewUrl: "https://lw.pages.dev" });
      const pr = w.find("[data-testid='pr-link']").element as HTMLAnchorElement;
      expect(pr.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }))).toBe(false);
      expect(open).toHaveBeenCalledWith(draft.url, "_blank", "noopener,noreferrer");

      const preview = w.find("[data-testid='preview-deploy-link']").element as HTMLAnchorElement;
      expect(preview.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }))).toBe(false);
      expect(open).toHaveBeenCalledWith("https://lw.pages.dev/", "_blank", "noopener,noreferrer");
    } finally {
      window.open = original;
    }
  });

  it("offers Ask for a greenlight only for a draft", async () => {
    expect((await mountWith(draft)).w.find(ready).exists()).toBe(true);
    expect((await mountWith({ ...draft, state: "open" })).w.find(ready).exists()).toBe(false);
  });

  it("marks the PR ready and trusts the returned state", async () => {
    vi.useFakeTimers();
    try {
      const markChangeReady = vi.fn(async () => ({ ...draft, state: "open" as const }));
      const changeStatus = vi.fn(async () => draft);
      const { w, store } = await mountWith(draft, { markChangeReady, changeStatus });
      await w.find(ready).trigger("click");
      await flushPromises();
      expect(markChangeReady).toHaveBeenCalledWith("site", "content/launch-week");
      expect(changeStatus).not.toHaveBeenCalled();
      expect(store.status?.state).toBe("open");
      // Selecting closes the menu; reopened, the open PR no longer offers it.
      await openMenu();
      expect(w.find(ready).exists()).toBe(false);
      changeStatus.mockResolvedValue({ ...draft, state: "open" });
      await vi.advanceTimersByTimeAsync(STATUS_POLL_MS);
      expect(changeStatus).toHaveBeenCalledTimes(1);
      store.stopStatusPolling();
    } finally {
      vi.useRealTimers();
    }
  });

  it("surfaces a 403 and keeps the PR a draft", async () => {
    const markChangeReady = vi.fn(async () => {
      throw new ApiError("SANDBOX", "You don't have push access", 403);
    });
    const { w, store } = await mountWith(draft, { markChangeReady });
    await w.find(ready).trigger("click");
    await flushPromises();
    expect(toast.error).toHaveBeenCalledWith("You don't have push access");
    expect(store.status?.state).toBe("draft");
  });

  it("prompts sign-in instead of a toast when GitHub needs it", async () => {
    const markChangeReady = vi.fn(async () => {
      throw new ApiError("UNAUTH", "your GitHub session expired — sign in again", 401);
    });
    const { w, store } = await mountWith(draft, { markChangeReady });
    await w.find(ready).trigger("click");
    await flushPromises();
    expect(store.needsSignIn).toBe(true);
    expect(toast.error).not.toHaveBeenCalled();
  });
});
