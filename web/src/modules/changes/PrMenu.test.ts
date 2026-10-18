// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { STATUS_POLL_MS } from "./store";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
import type { PrStatus } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

import { toast } from "vue-sonner";
import PrMenu from "./PrMenu.vue";
import { useChangesStore } from "./store";

const draft: PrStatus = {
  number: 4,
  url: "https://github.com/acme/site/pull/4",
  state: "draft",
  checks: "failing",
  previewUrl: null,
};

/** Render the menu inline (no portal, always open); items emit `select` on click. */
const stubs = {
  DropdownMenu: { template: "<div><slot /></div>" },
  DropdownMenuTrigger: { template: "<div><slot /></div>" },
  DropdownMenuContent: { template: "<div><slot /></div>" },
  DropdownMenuLabel: { template: "<div><slot /></div>" },
  DropdownMenuSeparator: { template: "<hr />" },
  DropdownMenuItem: {
    props: ["disabled", "asChild"],
    emits: ["select"],
    template: `<slot v-if="asChild" /><div v-else :data-disabled="disabled || undefined" @click="$emit('select')"><slot /></div>`,
  },
};

function mountWith(status: PrStatus | null, api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  const store = useChangesStore();
  store.projectId = "site";
  store.active = { branch: "content/launch-week", title: "Launch week" };
  store.prUrl = draft.url;
  store.status = status;
  return { w: mount(PrMenu, { props: { variant: "secondary" }, global: { stubs } }), store };
}

const ready = "[data-testid='ready-for-review']";

beforeEach(() => {
  vi.mocked(toast.error).mockClear();
});

describe("PrMenu", () => {
  it("shows the PR number and status", () => {
    const { w } = mountWith(draft);
    expect(w.find("[data-testid='pr-menu']").text()).toContain("PR #4");
    expect(w.find("[data-testid='pr-status']").text()).toBe("Draft · Checks failing");
  });

  it("links the PR before its status has loaded", () => {
    const { w } = mountWith(null);
    expect(w.find("[data-testid='pr-menu']").text()).toContain("Pull request");
    expect(w.find("[data-testid='pr-status']").exists()).toBe(false);
    expect(w.find("[data-testid='pr-link']").attributes("href")).toBe(draft.url);
  });

  it("links the deployed preview, never a non-http one", () => {
    const { w } = mountWith({ ...draft, previewUrl: "https://lw.pages.dev" });
    const a = w.find("[data-testid='preview-deploy-link']");
    expect(a.attributes("href")).toBe("https://lw.pages.dev");
    expect(a.attributes("rel")).toContain("noopener");
    for (const previewUrl of ["javascript:alert(1)", "//evil.dev"]) {
      expect(mountWith({ ...draft, previewUrl }).w.find("[data-testid='preview-deploy-link']").exists()).toBe(false);
    }
  });

  it("opens menu links in the browser instead of navigating the app", () => {
    const open = vi.fn();
    const original = window.open;
    window.open = open;
    try {
      const { w } = mountWith({ ...draft, previewUrl: "https://lw.pages.dev" });
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

  it("offers Ready for review only for a draft", () => {
    expect(mountWith(draft).w.find(ready).exists()).toBe(true);
    expect(mountWith({ ...draft, state: "open" }).w.find(ready).exists()).toBe(false);
  });

  it("marks the PR ready and trusts the returned state", async () => {
    vi.useFakeTimers();
    try {
      const markChangeReady = vi.fn(async () => ({ ...draft, state: "open" as const }));
      const changeStatus = vi.fn(async () => draft);
      const { w, store } = mountWith(draft, { markChangeReady, changeStatus });
      await w.find(ready).trigger("click");
      await flushPromises();
      expect(markChangeReady).toHaveBeenCalledWith("site", "content/launch-week");
      expect(changeStatus).not.toHaveBeenCalled();
      expect(store.status?.state).toBe("open");
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
    const { w, store } = mountWith(draft, { markChangeReady });
    await w.find(ready).trigger("click");
    await flushPromises();
    expect(toast.error).toHaveBeenCalledWith("You don't have push access");
    expect(store.status?.state).toBe("draft");
  });

  it("prompts sign-in instead of a toast when GitHub needs it", async () => {
    const markChangeReady = vi.fn(async () => {
      throw new ApiError("UNAUTH", "your GitHub session expired — sign in again", 401);
    });
    const { w, store } = mountWith(draft, { markChangeReady });
    await w.find(ready).trigger("click");
    await flushPromises();
    expect(store.needsSignIn).toBe(true);
    expect(toast.error).not.toHaveBeenCalled();
  });
});
