// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { ref } from "vue";
import { createFakeDaemon } from "@/test/fakeDaemon";
import type { SetupStatus } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import App from "./App.vue";
import { SETUP_WAIT_MS, dismissSetup } from "./app/setup";

const complete: SetupStatus = {
  git: true,
  gh: true,
  githubSignedIn: true,
  providers: [{ id: "claude", name: "Claude Code", available: true }],
  projectsDir: "/p",
};

function mountWith(status: SetupStatus | (() => Promise<SetupStatus>)) {
  const setupStatus = vi.fn(typeof status === "function" ? status : async () => status);
  h.daemon = {
    ...createFakeDaemon({
      setupStatus,
      githubStatus: vi.fn(async () => ({ configured: false, deviceFlow: false, source: null, user: null })),
    }),
    reason: ref(null),
    start: vi.fn(),
    stop: vi.fn(),
  };
  setActivePinia(createPinia());
  const w = mount(App, {
    global: { stubs: { RouterView: { template: "<main data-testid='shell' />" }, CommandPalette: true, Toaster: true } },
  });
  return { w, setupStatus };
}

enableAutoUnmount(afterEach);
afterEach(() => localStorage.clear());

describe("App setup gate", () => {
  it("shows the setup check when something is missing", async () => {
    const { w } = mountWith({ ...complete, gh: false });
    await flushPromises();
    expect(w.find("[data-testid='setup-check']").exists()).toBe(true);
    expect(w.find("[data-testid='shell']").exists()).toBe(false);
  });

  it("goes to the shell when everything is ready", async () => {
    const { w } = mountWith(complete);
    await flushPromises();
    expect(w.find("[data-testid='setup-check']").exists()).toBe(false);
    expect(w.find("[data-testid='shell']").exists()).toBe(true);
  });

  it("holds the shell back while the check is in flight", async () => {
    const { w } = mountWith(() => new Promise<SetupStatus>(() => {}));
    await flushPromises();
    expect(w.find("[data-testid='setup-pending']").exists()).toBe(true);
    expect(w.find("[data-testid='shell']").exists()).toBe(false);
    expect(w.find("[data-testid='setup-check']").exists()).toBe(false);
  });

  it("goes to the shell when the check fails", async () => {
    const { w } = mountWith(async () => {
      throw new Error("boom");
    });
    await flushPromises();
    expect(w.find("[data-testid='shell']").exists()).toBe(true);
    expect(w.text()).not.toContain("boom");
  });

  it("goes to the shell when the check takes too long", async () => {
    vi.useFakeTimers();
    try {
      const { w } = mountWith(() => new Promise<SetupStatus>(() => {}));
      await vi.advanceTimersByTimeAsync(SETUP_WAIT_MS);
      expect(w.find("[data-testid='shell']").exists()).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });

  it("opens the shell immediately once dismissed", () => {
    dismissSetup();
    const { w, setupStatus } = mountWith({ ...complete, gh: false });
    expect(w.find("[data-testid='shell']").exists()).toBe(true);
    expect(setupStatus).not.toHaveBeenCalled();
  });

  it("remembers the dismissal and opens the shell", async () => {
    const { w } = mountWith({ ...complete, gh: false });
    await flushPromises();
    await w.find("[data-testid='setup-continue']").trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='shell']").exists()).toBe(true);
    expect(localStorage.length).toBe(1);
  });
});
