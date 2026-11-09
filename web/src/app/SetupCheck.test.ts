// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { NetworkError } from "@/core/api";
import type { RuntimeStatus, SetupProvider, SetupStatus } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import SetupCheck from "./SetupCheck.vue";
import { PROVIDER_NAMES } from "@/modules/assistant/rows";

const runtime = (over: Partial<RuntimeStatus> = {}): RuntimeStatus => ({
  phase: "ready",
  version: "2.2.5",
  source: "managed",
  progress: null,
  message: null,
  downloadBytes: 157_286_400,
  ...over,
});

const notDownloaded = (): RuntimeStatus =>
  runtime({ phase: "notDownloaded", version: null, source: null, progress: null });

const provider = (id: string, over: Partial<SetupProvider> = {}): SetupProvider => ({
  id,
  name: PROVIDER_NAMES[id] ?? id,
  available: true,
  runtime: runtime(),
  installed: true,
  signedIn: true,
  chatReady: true,
  freeModels: false,
  reason: null,
  ...over,
});

const freshProvider = (id: string): SetupProvider =>
  provider(id, {
    available: false,
    runtime: notDownloaded(),
    installed: false,
    signedIn: null,
    chatReady: false,
    freeModels: id === "opencode",
  });

const status = (over: Partial<SetupStatus> = {}): SetupStatus => ({
  git: true,
  gh: true,
  githubSignedIn: true,
  node: runtime({ version: "24.0.0" }),
  providers: [provider("opencode", { freeModels: true, signedIn: null })],
  projectsDir: "/Users/ada/orso-projects",
  ...over,
});

const fresh = (): SetupStatus =>
  status({
    node: notDownloaded(),
    providers: ["opencode", "claude", "codex", "cursor"].map(freshProvider),
  });

const signedOut = { configured: true, deviceFlow: true, source: null, user: null };

function mountCheck(
  api: Record<string, unknown> = {},
  value: SetupStatus | (() => Promise<SetupStatus>) = status(),
) {
  h.daemon = createFakeDaemon({
    setupStatus: vi.fn(typeof value === "function" ? value : async () => value),
    // Keep the status authoritative: the empty runtime snapshot merges nothing over it.
    setupRuntimes: vi.fn(async () => ({ providers: [] })),
    githubStatus: vi.fn(async () => signedOut),
    ...api,
  });
  setActivePinia(createPinia());
  return mount(SetupCheck, { attachTo: document.body });
}

const testId = (w: ReturnType<typeof mount>, id: string) =>
  w.get(`[data-testid='${id}']`);

async function toSite(w: ReturnType<typeof mount>) {
  await testId(w, "onboarding-setup-opencode").trigger("click");
  await flushPromises();
}

async function toGithub(w: ReturnType<typeof mount>) {
  await toSite(w);
  await testId(w, "onboarding-next").trigger("click");
  await flushPromises();
}

enableAutoUnmount(afterEach);

describe("onboarding step 1: assistant", () => {
  it("renders the assistant step with four cards and OpenCode selected", async () => {
    const w = mountCheck({}, fresh());
    await flushPromises();

    expect(w.find("[data-testid='onboarding-step-assistant']").exists()).toBe(true);
    expect(w.find("[data-testid='onboarding-step-site']").exists()).toBe(false);
    expect(w.text()).toContain("Set up your assistant.");
    expect(w.text()).toContain(
      "Download an assistant to make changes to your site. OpenCode is free to start; use the account you already have for the others.",
    );

    const ids = ["opencode", "claude", "codex", "cursor"];
    for (const id of ids) expect(w.find(`[data-testid='onboarding-provider-${id}']`).exists()).toBe(true);

    expect(testId(w, "onboarding-provider-opencode").attributes("aria-checked")).toBe("true");
    for (const id of ["claude", "codex", "cursor"]) {
      expect(testId(w, `onboarding-provider-${id}`).attributes("aria-checked")).toBe("false");
    }
    expect(testId(w, "onboarding-setup-opencode").text()).toContain("Set up with OpenCode");
    expect(w.findAll("[data-testid^='onboarding-dot-']")).toHaveLength(3);
    expect(testId(w, "onboarding-dot-1").attributes("aria-current")).toBe("step");
    expect(w.find("[data-testid='onboarding-skip']").exists()).toBe(true);
  });

  it("shows the advanced existing-assistant block without commands", async () => {
    const w = mountCheck({}, fresh());
    await flushPromises();

    const advanced = w.get("[data-testid='onboarding-existing']");
    expect(advanced.text()).toContain("Or use an assistant you already have.");
    expect(advanced.text()).toContain("will appear here");
    expect(advanced.text()).not.toMatch(/npx|--|open code|terminal/i);
    expect(w.find("[data-testid='onboarding-existing-claude']").exists()).toBe(false);
  });

  it("lists system assistants that are ready and selects one", async () => {
    const w = mountCheck(
      {},
      status({
        providers: [
          provider("opencode", { freeModels: true, signedIn: null }),
          provider("claude", {
            runtime: runtime({ source: "system" }),
            signedIn: true,
            chatReady: true,
          }),
        ],
      }),
    );
    await flushPromises();

    const existing = testId(w, "onboarding-existing-claude");
    expect(existing.text()).toContain("Claude");
    expect(existing.text()).toContain("Already on your Mac");
    await existing.trigger("click");

    expect(testId(w, "onboarding-provider-claude").attributes("aria-checked")).toBe("true");
    expect(testId(w, "onboarding-setup-claude").text()).toContain("Continue");
  });

  it("changes the CTA when the selection changes", async () => {
    const w = mountCheck({}, fresh());
    await flushPromises();

    await testId(w, "onboarding-provider-claude").trigger("click");
    expect(testId(w, "onboarding-provider-claude").attributes("aria-checked")).toBe("true");
    expect(testId(w, "onboarding-provider-opencode").attributes("aria-checked")).toBe("false");
    expect(testId(w, "onboarding-setup-claude").text()).toContain("Set up with Claude");
    expect(w.findAll("[data-testid='onboarding-selected-check']")).toHaveLength(1);
  });

  it("starts a download, shows progress and cancels it", async () => {
    const downloadRuntime = vi.fn(async () => runtime({ phase: "downloading", progress: 0 }));
    const cancelRuntime = vi.fn(async () => notDownloaded());
    const w = mountCheck({ downloadRuntime, cancelRuntime }, fresh());
    await flushPromises();

    await testId(w, "onboarding-setup-opencode").trigger("click");
    await flushPromises();

    expect(downloadRuntime).toHaveBeenCalledWith("opencode");
    expect(w.find("[data-testid='onboarding-progress-opencode']").exists()).toBe(true);
    expect(testId(w, "onboarding-setup-opencode").text()).toContain("Downloading…");
    expect(testId(w, "onboarding-cancel-opencode").text()).toContain("Cancel");

    h.daemon.emit({
      type: "setup.runtime",
      runtime: "opencode",
      phase: "downloading",
      progress: 42,
    });
    await nextTick();
    expect(testId(w, "onboarding-percent-opencode").text()).toContain("42%");

    await testId(w, "onboarding-cancel-opencode").trigger("click");
    await flushPromises();
    expect(cancelRuntime).toHaveBeenCalledWith("opencode");
    expect(w.find("[data-testid='onboarding-progress-opencode']").exists()).toBe(false);
    expect(testId(w, "onboarding-setup-opencode").text()).toContain("Set up with OpenCode");
  });

  it("offers Retry after a failed download", async () => {
    const retryRuntime = vi.fn(async () => runtime({ phase: "downloading", progress: 3 }));
    const w = mountCheck(
      { retryRuntime },
      status({
        providers: [
          provider("opencode", {
            runtime: runtime({ phase: "downloadError", version: null, message: "The connection dropped." }),
            installed: false,
            chatReady: false,
            freeModels: true,
            signedIn: null,
          }),
        ],
      }),
    );
    await flushPromises();

    expect(testId(w, "onboarding-setup-opencode").text()).toContain("Retry");
    expect(testId(w, "onboarding-error-opencode").text()).toContain(
      "We couldn't download OpenCode. Check your connection and try again.",
    );

    await testId(w, "onboarding-setup-opencode").trigger("click");
    await flushPromises();
    expect(retryRuntime).toHaveBeenCalledWith("opencode");
    expect(testId(w, "onboarding-setup-opencode").text()).toContain("Downloading…");
  });

  it("asks a downloaded assistant to sign in", async () => {
    const signInProvider = vi.fn(async () => ({
      state: "browser" as const,
      url: "https://example.test/login",
    }));
    const w = mountCheck(
      { signInProvider },
      status({
        providers: [
          provider("opencode", { freeModels: true, signedIn: null }),
          provider("claude", { signedIn: false, chatReady: false }),
        ],
      }),
    );
    await flushPromises();

    await testId(w, "onboarding-provider-claude").trigger("click");
    expect(testId(w, "onboarding-setup-claude").text()).toContain("Sign in");
    expect(w.find("[data-testid='assistant-sign-in']").exists()).toBe(false);

    await testId(w, "onboarding-setup-claude").trigger("click");
    await flushPromises();
    expect(signInProvider).toHaveBeenCalledWith("claude");
    const panel = w.get("[data-testid='assistant-sign-in']");
    expect(panel.attributes("data-state")).toBe("browser");
    expect(panel.text()).toContain("Finish signing in to Claude");
  });

  it("moves on when the assistant is already ready", async () => {
    const w = mountCheck();
    await flushPromises();

    expect(testId(w, "onboarding-setup-opencode").text()).toContain("Continue");
    expect(testId(w, "onboarding-next").attributes("disabled")).toBeUndefined();
    await toSite(w);
    expect(w.find("[data-testid='onboarding-step-site']").exists()).toBe(true);
    expect(w.find("[data-testid='onboarding-step-assistant']").exists()).toBe(false);
  });

  it("skips setup and rechecks", async () => {
    let next = fresh();
    const setupStatus = vi.fn(async () => next);
    const setupRuntimes = vi.fn(async () => ({ providers: [] }));
    const w = mountCheck({ setupStatus, setupRuntimes }, async () => next);
    await flushPromises();

    await testId(w, "setup-recheck").trigger("click");
    await flushPromises();
    expect(setupStatus).toHaveBeenCalled();
    expect(setupRuntimes).toHaveBeenCalled();

    next = status();
    await testId(w, "setup-recheck").trigger("click");
    await flushPromises();
    expect(testId(w, "onboarding-setup-opencode").text()).toContain("Continue");

    await testId(w, "onboarding-skip").trigger("click");
    expect(w.emitted("continue")).toHaveLength(1);
  });
});

describe("onboarding step 2: site builder", () => {
  it("gates Continue on the site builder and downloads it", async () => {
    const downloadRuntime = vi.fn(async () => runtime({ phase: "downloading", progress: 0 }));
    const cancelRuntime = vi.fn(async () => notDownloaded());
    const w = mountCheck(
      { downloadRuntime, cancelRuntime },
      status({
        providers: [provider("opencode", { freeModels: true, signedIn: null })],
        node: notDownloaded(),
      }),
    );
    await flushPromises();

    await toSite(w);
    expect(w.text()).toContain("Add the site builder.");
    expect(w.text()).toContain(
      "orso uses this to install packages and preview your site. It is downloaded once.",
    );
    expect(testId(w, "onboarding-site-primary").text()).toContain("Download");
    expect(testId(w, "onboarding-next").attributes("disabled")).toBeDefined();

    await testId(w, "onboarding-site-primary").trigger("click");
    await flushPromises();
    expect(downloadRuntime).toHaveBeenCalledWith("node");
    expect(w.find("[data-testid='onboarding-site-progress']").exists()).toBe(true);
    expect(testId(w, "onboarding-site-primary").text()).toContain("Cancel");

    h.daemon.emit({ type: "setup.runtime", runtime: "node", phase: "downloading", progress: 65 });
    await nextTick();
    expect(testId(w, "onboarding-site-progress").text()).toContain("65%");

    await testId(w, "onboarding-site-primary").trigger("click");
    await flushPromises();
    expect(cancelRuntime).toHaveBeenCalledWith("node");

    h.daemon.emit({
      type: "setup.runtime",
      runtime: "node",
      phase: "ready",
      version: "24.0.0",
      source: "managed",
    });
    await nextTick();
    expect(testId(w, "onboarding-next").attributes("disabled")).toBeUndefined();

    await testId(w, "onboarding-next").trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='onboarding-step-github']").exists()).toBe(true);
  });

  it("retries a failed site-builder download", async () => {
    const retryRuntime = vi.fn(async () => runtime({ phase: "downloading", progress: 1 }));
    const w = mountCheck(
      { retryRuntime },
      status({
        providers: [provider("opencode", { freeModels: true, signedIn: null })],
        node: runtime({ phase: "downloadError", version: null, message: "Network" }),
      }),
    );
    await flushPromises();
    await toSite(w);

    expect(testId(w, "onboarding-site-primary").text()).toContain("Retry");
    expect(testId(w, "onboarding-site-error").text()).toContain(
      "We couldn't download the site builder. Check your connection and try again.",
    );
    await testId(w, "onboarding-site-primary").trigger("click");
    await flushPromises();
    expect(retryRuntime).toHaveBeenCalledWith("node");
    expect(testId(w, "onboarding-site-primary").text()).toContain("Cancel");
  });

  it("shows the site-builder version and source when ready", async () => {
    const w = mountCheck();
    await flushPromises();
    await toSite(w);

    expect(testId(w, "onboarding-site-primary").text()).toContain("Continue");
    expect(w.text()).toContain("Version 24.0.0");
    expect(w.text()).toContain("Downloaded by orso");
    expect(testId(w, "onboarding-next").attributes("disabled")).toBeUndefined();
  });
});

describe("onboarding step 3: GitHub", () => {
  it("keeps GitHub optional and skips it", async () => {
    const w = mountCheck();
    await flushPromises();
    await toGithub(w);

    expect(w.text()).toContain("Connect GitHub.");
    expect(w.text()).toContain(
      "Share previews and publish when you are ready. You can do this later.",
    );
    expect(w.find("[data-testid='onboarding-git']").exists()).toBe(true);
    expect(w.find("[data-testid='onboarding-github-tools']").exists()).toBe(true);
    expect(w.find("[data-testid='onboarding-github-account']").exists()).toBe(true);

    await testId(w, "onboarding-github-skip").trigger("click");
    expect(w.emitted("continue")).toHaveLength(1);
  });

  it("finishes with Start building", async () => {
    const w = mountCheck();
    await flushPromises();
    await toGithub(w);

    expect(testId(w, "onboarding-next").text()).toContain("Start building");
    await testId(w, "onboarding-next").trigger("click");
    expect(w.emitted("continue")).toHaveLength(1);
  });

  it("offers the GitHub sign-in when the account is missing", async () => {
    const w = mountCheck({}, status({ githubSignedIn: false }));
    await flushPromises();
    await toGithub(w);

    expect(testId(w, "onboarding-github-account").text()).toContain(
      "Sign in so orso can open your sites and share your changes.",
    );
    expect(w.find("[data-testid='setup-sign-in']").exists()).toBe(true);
  });

  it("walks back through the steps", async () => {
    const w = mountCheck();
    await flushPromises();
    await toGithub(w);

    await testId(w, "onboarding-back").trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='onboarding-step-site']").exists()).toBe(true);

    await testId(w, "onboarding-back").trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='onboarding-step-assistant']").exists()).toBe(true);
    expect(testId(w, "onboarding-back").attributes("disabled")).toBeDefined();
  });

  it("lets a completed dot jump back", async () => {
    const w = mountCheck();
    await flushPromises();
    await toGithub(w);

    await testId(w, "onboarding-dot-1").trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='onboarding-step-assistant']").exists()).toBe(true);
  });
});

describe("onboarding copy and failures", () => {
  it("never shows developer words in any step", async () => {
    const w = mountCheck();
    await flushPromises();

    const banned = /\b(agent|cli|acp|sdk|npx|path|runtime)\b|vibe coding/i;
    expect(w.text()).not.toMatch(banned);

    await toSite(w);
    expect(w.text()).not.toMatch(banned);

    await testId(w, "onboarding-next").trigger("click");
    await flushPromises();
    expect(w.find("[data-testid='onboarding-step-github']").exists()).toBe(true);
    expect(w.text()).not.toMatch(banned);
  });

  it("shows a friendly message, not the raw error, when the check fails", async () => {
    const w = mountCheck({}, async () => {
      throw new NetworkError("cannot reach daemon at http://127.0.0.1:7800");
    });
    await flushPromises();

    expect(w.find("[data-testid='setup-error']").exists()).toBe(true);
    expect(w.text()).not.toContain("cannot reach daemon");
    expect(w.text()).not.toContain("127.0.0.1");
  });
});
