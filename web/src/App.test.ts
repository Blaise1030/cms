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
import { dismissSetup } from "./app/setup";

const complete: SetupStatus = {
  git: true,
  gh: true,
  githubSignedIn: true,
  providers: [{ id: "claude", name: "Claude Code", available: true }],
  projectsDir: "/p",
};

function mountWith(status: SetupStatus) {
  const setupStatus = vi.fn(async () => status);
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

  it("skips the check once dismissed", async () => {
    dismissSetup();
    const { w, setupStatus } = mountWith({ ...complete, gh: false });
    await flushPromises();
    expect(setupStatus).not.toHaveBeenCalled();
    expect(w.find("[data-testid='shell']").exists()).toBe(true);
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
