// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { NetworkError } from "@/core/api";
import type { SetupProvider, SetupStatus } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import SetupCheck from "./SetupCheck.vue";

const provider = (id: string, name: string, over: Partial<SetupProvider> = {}): SetupProvider => ({
  id,
  name,
  available: true,
  chatReady: true,
  ...over,
});

const allMissing: SetupStatus = {
  git: false,
  gh: false,
  githubSignedIn: false,
  providers: [
    provider("cursor", "Cursor", { available: false, chatReady: false }),
    provider("claude", "Claude", { available: false, chatReady: false }),
    provider("codex", "Codex", { available: false, chatReady: false }),
  ],
  projectsDir: "/Users/ada/orso-projects",
};

const ready: SetupStatus = {
  ...allMissing,
  git: true,
  gh: true,
  githubSignedIn: true,
  providers: allMissing.providers.map((p) => ({
    ...p,
    available: true,
    chatReady: p.id === "claude",
  })),
};

const signedOut = { configured: true, deviceFlow: true, source: null, user: null };

function mountWith(setupStatus: () => Promise<SetupStatus>) {
  const api = {
    setupStatus: vi.fn(setupStatus),
    setupRuntimes: vi.fn(async () => ({ providers: [] })),
    githubStatus: vi.fn(async () => signedOut),
  };
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return { w: mount(SetupCheck, { attachTo: document.body }), api };
}

const row = (w: ReturnType<typeof mount>, item: string) => w.find(`[data-testid='setup-${item}']`);

enableAutoUnmount(afterEach);

describe("SetupCheck", () => {
  it("asks for git, the GitHub CLI and an assistant when everything is missing", async () => {
    const { w } = mountWith(async () => allMissing);
    await flushPromises();
    for (const item of ["git", "gh"]) {
      expect(row(w, item).text()).toContain("Action needed");
      expect(row(w, item).find("a[target='_blank'][rel='noopener noreferrer']").exists()).toBe(true);
    }
    expect(row(w, "github").text()).toContain("Action needed");
    const assistant = row(w, "assistant");
    expect(assistant.text()).toContain("Action needed");
    // The assistant is downloaded in place, not linked out to vendor pages.
    expect(assistant.find("[data-testid='assistant-setup']").exists()).toBe(true);
    expect(assistant.find("a[target='_blank']").exists()).toBe(false);
    expect(w.text()).toContain("/Users/ada/orso-projects");
  });

  it("shows every item ready", async () => {
    const { w } = mountWith(async () => ready);
    await flushPromises();
    for (const item of ["git", "gh", "github", "assistant"]) {
      expect(row(w, item).text()).toContain("Ready");
      expect(row(w, item).text()).not.toContain("Action needed");
    }
    expect(row(w, "assistant").text()).toContain("Claude");
    expect(row(w, "assistant").find("[data-testid='assistant-setup']").exists()).toBe(false);
  });

  it("reads assistant readiness from chatReady, not available", async () => {
    const installedOnly = mountWith(async () => ({
      ...ready,
      providers: [provider("claude", "Claude", { available: true, chatReady: false })],
    }));
    await flushPromises();
    expect(row(installedOnly.w, "assistant").text()).toContain("Action needed");
    installedOnly.w.unmount();

    const readyButUnavailable = mountWith(async () => ({
      ...ready,
      providers: [provider("opencode", "OpenCode", { available: false, chatReady: true })],
    }));
    await flushPromises();
    expect(row(readyButUnavailable.w, "assistant").text()).toContain("Ready");
    readyButUnavailable.w.unmount();
  });

  it("rechecks the setup status and the assistant snapshot on request", async () => {
    let next = allMissing;
    const { w, api } = mountWith(async () => next);
    await flushPromises();
    next = ready;
    await w.find("[data-testid='setup-recheck']").trigger("click");
    await flushPromises();
    expect(api.setupStatus.mock.calls.length).toBeGreaterThanOrEqual(2);
    expect(api.setupRuntimes.mock.calls.length).toBeGreaterThanOrEqual(2);
    expect(row(w, "git").text()).toContain("Ready");
    expect(row(w, "assistant").text()).toContain("Ready");
  });

  it("shows a friendly message, not the raw error, when the check fails", async () => {
    const { w } = mountWith(async () => {
      throw new NetworkError("cannot reach daemon at http://127.0.0.1:7800");
    });
    await flushPromises();
    expect(w.find("[data-testid='setup-error']").exists()).toBe(true);
    expect(w.text()).not.toContain("cannot reach daemon");
    expect(w.text()).not.toContain("127.0.0.1");
  });

  it("emits continue when the user moves on", async () => {
    const { w } = mountWith(async () => allMissing);
    await flushPromises();
    await w.find("[data-testid='setup-continue']").trigger("click");
    expect(w.emitted("continue")).toHaveLength(1);
  });
});
