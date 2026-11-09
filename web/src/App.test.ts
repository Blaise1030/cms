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
  providers: [{ id: "claude", name: "Claude", available: true, chatReady: true }],
  projectsDir: "/p",
};

function mountWith(status: SetupStatus | (() => Promise<SetupStatus>)) {
  const setupStatus = vi.fn(typeof status === "function" ? status : async () => status);
  h.daemon = {
    ...createFakeDaemon({
      setupStatus,
      dismissSetup: vi.fn(async () => ({ dismissed: true as const })),
      githubStatus: vi.fn(async () => ({ configured: false, deviceFlow: false, source: null, user: null })),
    }),
    reason: ref(null),
    start: vi.fn(),
    stop: vi.fn(),
  };
  setActivePinia(createPinia());
  const w = mount(App, {
    global: { stubs: { RouterView: { template: "<main data-testid='shell' />" }, CommandPalette: true, AssistantDialog: true, Toaster: true } },
  });
  return { w, setupStatus };
}

enableAutoUnmount(afterEach);
afterEach(() => localStorage.clear());

describe("App setup gate", () => {
  it("shows the setup check when something required is missing", async () => {
    const { w } = mountWith({
      ...complete,
      providers: [{ id: "claude", name: "Claude", available: true, chatReady: false }],
    });
    await flushPromises();
    expect(w.find("[data-testid='setup-check']").exists()).toBe(true);
    expect(w.find("[data-testid='shell']").exists()).toBe(false);
  });

  it("opens the shell when only optional GitHub pieces are missing", async () => {
    const { w } = mountWith({ ...complete, gh: false, githubSignedIn: false });
    await flushPromises();
    expect(w.find("[data-testid='setup-check']").exists()).toBe(false);
    expect(w.find("[data-testid='shell']").exists()).toBe(true);
  });

  it("honors a daemon-backed dismiss even when setup is still incomplete", async () => {
    const { w } = mountWith({
      ...complete,
      setupDismissed: true,
      providers: [{ id: "claude", name: "Claude", available: true, chatReady: false }],
    });
    await flushPromises();
    expect(w.find("[data-testid='setup-check']").exists()).toBe(false);
    expect(w.find("[data-testid='shell']").exists()).toBe(true);
  });

  it("shows the setup check when an assistant is installed but not ready to chat", async () => {
    const { w } = mountWith({
      ...complete,
      providers: [{ id: "claude", name: "Claude", available: true, chatReady: false }],
    });
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
    const { w, setupStatus } = mountWith({
      ...complete,
      providers: [{ id: "claude", name: "Claude", available: true, chatReady: false }],
    });
    expect(w.find("[data-testid='shell']").exists()).toBe(true);
    expect(setupStatus).not.toHaveBeenCalled();
  });

  it("remembers the dismissal and opens the shell", async () => {
    const { w } = mountWith({
      ...complete,
      providers: [{ id: "claude", name: "Claude", available: true, chatReady: false }],
    });
    await flushPromises();
    await w.find("[data-testid='onboarding-skip']").trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='shell']").exists()).toBe(true);
    expect(localStorage.length).toBe(1);
  });
});
