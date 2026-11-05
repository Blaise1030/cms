// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { SetupProvider } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import AssistantDialog from "./AssistantDialog.vue";
import { assistantDialogOpen } from "./dialog";

const provider = (over: Partial<SetupProvider> = {}): SetupProvider => ({
  id: "opencode",
  name: "OpenCode",
  available: true,
  runtime: { phase: "ready", version: "1.0.0", source: "managed", progress: null, message: null },
  installed: true,
  signedIn: true,
  chatReady: true,
  freeModels: false,
  reason: null,
  ...over,
});

function mountDialog(providers: SetupProvider[] = [provider()]) {
  h.daemon = createFakeDaemon({
    setupStatus: vi.fn(async () => ({
      git: true,
      gh: true,
      githubSignedIn: true,
      providers,
      projectsDir: "/p",
    })),
    setupRuntimes: vi.fn(async () => ({ providers: [] })),
  });
  setActivePinia(createPinia());
  assistantDialogOpen.value = true;
  return mount(AssistantDialog, { attachTo: document.body });
}

const el = (testId: string) =>
  document.body.querySelector<HTMLElement>(`[data-testid='${testId}']`);

enableAutoUnmount(afterEach);
beforeEach(() => {
  assistantDialogOpen.value = false;
});

describe("AssistantDialog", () => {
  it("shows the assistant setup and closes on Continue", async () => {
    mountDialog();
    await flushPromises();

    expect(document.body.textContent).toContain("Set up your assistant");
    expect(el("assistant-setup")).not.toBeNull();

    el("assistant-continue")!.click();
    await flushPromises();
    expect(assistantDialogOpen.value).toBe(false);
  });

  it("emits connect for a ready assistant and closes", async () => {
    const w = mountDialog();
    await flushPromises();

    el("assistant-action-opencode")!.click();
    await flushPromises();
    expect(w.emitted("connect")).toEqual([["opencode"]]);
    expect(assistantDialogOpen.value).toBe(false);
  });

  it("says nothing developer-facing", async () => {
    mountDialog([provider({ chatReady: false, signedIn: false })]);
    await flushPromises();
    expect(document.body.textContent).toContain("Sign-in required");
    expect(document.body.textContent).not.toMatch(/\b(agent|cli|acp|sdk|npx|path|runtime)\b/i);
  });
});
