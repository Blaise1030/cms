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

import AssistantSetup from "./AssistantSetup.vue";
import { PROVIDER_NAMES } from "./rows";
import type { AssistantSnapshot } from "./store";

const runtime = (over: Partial<RuntimeStatus> = {}): RuntimeStatus => ({
  phase: "ready",
  version: "2.2.5",
  source: "managed",
  progress: null,
  message: null,
  downloadBytes: 157_286_400,
  ...over,
});

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

const status = (over: Partial<SetupStatus> = {}): SetupStatus => ({
  git: true,
  gh: true,
  githubSignedIn: true,
  node: runtime({ version: "24.0.0" }),
  providers: [provider("opencode")],
  projectsDir: "/Users/ada/orso-projects",
  ...over,
});

const snapshot = (over: Partial<AssistantSnapshot> = {}): AssistantSnapshot => ({
  node: runtime({ version: "24.0.0" }),
  providers: [provider("opencode")],
  ...over,
});

/** Mount with a fresh pinia and a fake daemon; the store loads on mount. */
function mountSetup(api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon({
    setupStatus: vi.fn(async () => status()),
    // Keep the status authoritative; `mergeSnapshot` lets the runtime snapshot
    // override download fields, and these tests set those on the status.
    setupRuntimes: vi.fn(async () => snapshot({ providers: [] })),
    ...api,
  });
  setActivePinia(createPinia());
  return mount(AssistantSetup);
}

enableAutoUnmount(afterEach);

describe("AssistantSetup ordering", () => {
  it("puts OpenCode first, then the plan assistants, then the site builder", async () => {
    const w = mountSetup({
      setupStatus: vi.fn(async () =>
        status({
          providers: [
            provider("cursor"),
            provider("claude"),
            provider("codex"),
            provider("opencode", { freeModels: true, signedIn: null }),
          ],
        }),
      ),
    });
    await flushPromises();

    const ids = w.findAll("[data-testid^='assistant-row-']").map((el) => el.attributes("data-testid"));
    expect(ids).toEqual([
      "assistant-row-opencode",
      "assistant-row-codex",
      "assistant-row-claude",
      "assistant-row-cursor",
      "assistant-row-node",
    ]);

    const first = w.get("[data-testid='assistant-row-opencode']");
    expect(first.text()).toContain("Free models, no account needed.");
    expect(first.text()).toContain("Recommended");
    expect(first.text()).toContain("Connected");
  });

  it("keeps the site builder in its own section", async () => {
    const w = mountSetup();
    await flushPromises();

    const site = w.get("[data-testid='assistant-site-builder']");
    expect(site.text()).toContain("Site builder");
    expect(site.text()).toContain("Lets orso build and preview your site.");
    expect(site.text()).toContain("orso needs this to create a project and preview your site.");
    expect(w.get("[data-testid='assistant-assistants']").text()).not.toContain("Site builder");
  });

  it("offers an advanced disclosure for an assistant the user installed", async () => {
    const w = mountSetup();
    await flushPromises();

    expect(w.get("[data-testid='assistant-installed'] summary").text()).toBe(
      "I already installed an assistant",
    );
    expect(w.get("[data-testid='assistant-installed']").text()).toContain(
      "orso can use an assistant you installed yourself.",
    );
  });
});

describe("AssistantSetup props fallback", () => {
  it("renders from `initial` without asking the daemon", async () => {
    const daemon = createFakeDaemon();
    h.daemon = daemon;
    setActivePinia(createPinia());

    const w = mount(AssistantSetup, {
      props: {
        initial: {
          node: runtime({ version: "24.0.0" }),
          providers: [provider("claude"), provider("opencode", { freeModels: true, signedIn: null })],
        },
      },
    });
    await flushPromises();

    expect(daemon.api).not.toHaveProperty("setupStatus");
    expect(
      w.findAll("[data-testid^='assistant-row-']").map((el) => el.attributes("data-testid")),
    ).toEqual(["assistant-row-opencode", "assistant-row-claude", "assistant-row-node"]);
    expect(w.get("[data-testid='assistant-row-opencode']").text()).toContain("Free models");
  });

  it("emits continue only when the caller asks for the button", async () => {
    const daemon = createFakeDaemon();
    h.daemon = daemon;
    setActivePinia(createPinia());

    const plain = mount(AssistantSetup, { props: { initial: snapshot() } });
    expect(plain.find("[data-testid='assistant-continue']").exists()).toBe(false);

    const withButton = mount(AssistantSetup, {
      props: { initial: snapshot(), showContinue: true },
    });
    await withButton.get("[data-testid='assistant-continue']").trigger("click");
    expect(withButton.emitted("continue")).toHaveLength(1);
  });
});

describe("AssistantSetup downloads", () => {
  it("downloads, shows progress and cancels", async () => {
    const downloadRuntime = vi.fn(async () => runtime({ phase: "downloading", progress: 0 }));
    const cancelRuntime = vi.fn(async () =>
      runtime({ phase: "notDownloaded", version: null, source: null }),
    );
    const w = mountSetup({
      setupStatus: vi.fn(async () =>
        status({
          providers: [
            provider("opencode", {
              runtime: runtime({ phase: "notDownloaded", version: null, source: null }),
              installed: false,
              chatReady: false,
              freeModels: true,
              signedIn: null,
            }),
          ],
        }),
      ),
      downloadRuntime,
      cancelRuntime,
    });
    await flushPromises();

    const row = () => w.get("[data-testid='assistant-row-opencode']");
    const action = () => w.get("[data-testid='assistant-action-opencode']");
    expect(action().text()).toBe("Download");

    await action().trigger("click");
    await flushPromises();
    expect(downloadRuntime).toHaveBeenCalledWith("opencode");
    expect(row().text()).toContain("0%");
    expect(w.find("[data-testid='assistant-progress-opencode']").exists()).toBe(true);
    expect(action().text()).toBe("Cancel");

    h.daemon.emit({ type: "setup.runtime", runtime: "opencode", phase: "downloading", progress: 42 });
    await nextTick();
    expect(row().text()).toContain("42%");

    await action().trigger("click");
    await flushPromises();
    expect(cancelRuntime).toHaveBeenCalledWith("opencode");
    expect(action().text()).toBe("Download");
    expect(w.find("[data-testid='assistant-progress-opencode']").exists()).toBe(false);
  });

  it("shows a failed download and retries it", async () => {
    const retryRuntime = vi.fn(async () => runtime({ phase: "downloading", progress: 3 }));
    const w = mountSetup({
      setupStatus: vi.fn(async () =>
        status({
          providers: [
            provider("opencode", {
              runtime: runtime({
                phase: "downloadError",
                version: null,
                message: "The connection dropped.",
              }),
              installed: false,
              chatReady: false,
              freeModels: true,
              signedIn: null,
            }),
          ],
        }),
      ),
      retryRuntime,
    });
    await flushPromises();

    const row = w.get("[data-testid='assistant-row-opencode']");
    expect(row.text()).toContain("Download failed");
    const error = w.get("[data-testid='assistant-error-opencode']");
    expect(error.text()).toContain("We couldn't download OpenCode. Check your connection and try again.");
    expect(error.get("details").text()).toContain("The connection dropped.");

    await w.get("[data-testid='assistant-action-opencode']").trigger("click");
    await flushPromises();
    expect(retryRuntime).toHaveBeenCalledWith("opencode");
    expect(w.get("[data-testid='assistant-action-opencode']").text()).toBe("Cancel");
  });

  it("retries the whole setup check when loading fails", async () => {
    const setupStatus = vi
      .fn()
      .mockRejectedValueOnce(new NetworkError("cannot reach daemon at http://127.0.0.1:7800"))
      .mockResolvedValueOnce(status());
    const w = mountSetup({ setupStatus });
    await flushPromises();

    const error = w.get("[data-testid='assistant-load-error']");
    expect(error.text()).toContain("We couldn't check your setup just now.");
    expect(w.text()).not.toContain("127.0.0.1");

    await w.get("[data-testid='assistant-load-retry']").trigger("click");
    await flushPromises();
    expect(setupStatus).toHaveBeenCalledTimes(2);
    expect(w.find("[data-testid='assistant-load-error']").exists()).toBe(false);
    expect(w.find("[data-testid='assistant-row-opencode']").exists()).toBe(true);
  });

  it("checks again when the user says they already installed one", async () => {
    const setupStatus = vi.fn(async () => status());
    const w = mountSetup({ setupStatus });
    await flushPromises();

    await w.get("[data-testid='assistant-recheck']").trigger("click");
    await flushPromises();
    expect(setupStatus).toHaveBeenCalledTimes(2);
  });
});

describe("AssistantSetup sign-in", () => {
  it("waits for a browser sign-in and cancels it", async () => {
    const cancelProviderSignIn = vi.fn(async () => ({ state: "cancelled" as const }));
    const w = mountSetup({
      setupStatus: vi.fn(async () =>
        status({ providers: [provider("claude", { signedIn: false, chatReady: false })] }),
      ),
      signInProvider: vi.fn(async () => ({
        state: "browser" as const,
        url: "https://example.test/login",
      })),
      cancelProviderSignIn,
    });
    await flushPromises();

    expect(w.get("[data-testid='assistant-row-claude']").text()).toContain("Sign-in required");
    await w.get("[data-testid='assistant-action-claude']").trigger("click");
    await flushPromises();

    const panel = w.get("[data-testid='assistant-sign-in']");
    expect(panel.attributes("data-state")).toBe("browser");
    expect(panel.text()).toContain("Finish signing in to Claude");
    expect(panel.get("[data-testid='assistant-sign-in-open']").attributes("href")).toBe(
      "https://example.test/login",
    );

    await panel.get("[data-testid='assistant-sign-in-cancel']").trigger("click");
    await flushPromises();
    expect(cancelProviderSignIn).toHaveBeenCalledWith("claude");
    expect(w.find("[data-testid='assistant-sign-in']").exists()).toBe(false);
  });

  it("asks for the code from the sign-in page and submits it", async () => {
    const submitProviderSignInCode = vi.fn(async () => ({ state: "connected" as const }));
    const w = mountSetup({
      setupStatus: vi.fn(async () =>
        status({ providers: [provider("claude", { signedIn: false, chatReady: false })] }),
      ),
      signInProvider: vi.fn(async () => ({
        state: "code" as const,
        needsCode: true,
        url: "https://example.test/login",
      })),
      submitProviderSignInCode,
    });
    await flushPromises();

    await w.get("[data-testid='assistant-action-claude']").trigger("click");
    await flushPromises();

    const panel = w.get("[data-testid='assistant-sign-in']");
    expect(panel.attributes("data-state")).toBe("code");
    expect(panel.text()).toContain("Enter the code to sign in to Claude");

    const code = panel.get("[data-testid='assistant-sign-in-code']");
    expect(code.attributes("aria-label")).toBe("Sign-in code");
    await code.setValue("ABCD-1234");
    await panel.get("form").trigger("submit");
    await flushPromises();

    expect(submitProviderSignInCode).toHaveBeenCalledWith("claude", "ABCD-1234");
    expect(w.find("[data-testid='assistant-sign-in']").exists()).toBe(false);
    expect(w.get("[data-testid='assistant-row-claude']").text()).toContain("Connected");
  });

  it("shows why a sign-in failed and tries again", async () => {
    const signInProvider = vi
      .fn()
      .mockResolvedValueOnce({ state: "failed" as const, message: "The browser was closed." })
      .mockResolvedValueOnce({ state: "browser" as const, url: "https://example.test/login" });
    const w = mountSetup({
      setupStatus: vi.fn(async () =>
        status({ providers: [provider("claude", { signedIn: false, chatReady: false })] }),
      ),
      signInProvider,
    });
    await flushPromises();

    await w.get("[data-testid='assistant-action-claude']").trigger("click");
    await flushPromises();

    const error = w.get("[data-testid='assistant-sign-in-error']");
    expect(error.text()).toContain("Signing in to Claude didn't finish. Please try again.");
    expect(error.get("details").text()).toContain("The browser was closed.");

    await w.get("[data-testid='assistant-sign-in-retry']").trigger("click");
    await flushPromises();
    expect(signInProvider).toHaveBeenCalledTimes(2);
    expect(w.get("[data-testid='assistant-sign-in']").attributes("data-state")).toBe("browser");
  });

  it("does not gate OpenCode on a sign-in", async () => {
    const w = mountSetup({
      setupStatus: vi.fn(async () =>
        status({
          providers: [
            provider("opencode", { signedIn: null, chatReady: true, freeModels: true }),
          ],
        }),
      ),
    });
    await flushPromises();

    const row = w.get("[data-testid='assistant-row-opencode']");
    expect(row.text()).toContain("Connected");
    expect(row.text()).not.toContain("Sign-in required");
    expect(w.get("[data-testid='assistant-action-opencode']").text()).toBe("Sign in");
  });
});

describe("AssistantSetup copy", () => {
  it("never shows developer words", async () => {
    const w = mountSetup({
      setupStatus: vi.fn(async () =>
        status({
          providers: [
            provider("opencode", {
              runtime: runtime({
                phase: "downloadError",
                version: null,
                message: "The connection dropped.",
              }),
              chatReady: false,
            }),
            provider("codex", {
              runtime: runtime({ phase: "downloading", progress: 12 }),
              chatReady: false,
            }),
            provider("claude", {
              runtime: runtime({ phase: "finishing" }),
              chatReady: false,
              signedIn: null,
            }),
            provider("cursor", { runtime: runtime({ phase: "updateAvailable" }), signedIn: false }),
          ],
        }),
      ),
    });
    await flushPromises();

    expect(w.text()).toContain("assistant");
    expect(w.text()).not.toMatch(/\b(agent|cli|acp|sdk|npx|path|runtime)\b/i);
  });
});
