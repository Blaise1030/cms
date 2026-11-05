import { describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type {
  ProviderSignInState,
  RuntimeStatus,
  SetupProvider,
  SetupSnapshot,
  SetupStatus,
} from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useAssistantStore } from "./store";

const runtime = (over: Partial<RuntimeStatus> = {}): RuntimeStatus => ({
  phase: "ready",
  version: "2.2.5",
  source: "managed",
  progress: null,
  message: null,
  ...over,
});

const provider = (id: string, over: Partial<SetupProvider> = {}): SetupProvider => ({
  id,
  name: id,
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
  projectsDir: "/projects",
  ...over,
});

const snapshot = (over: Partial<SetupSnapshot> = {}): SetupSnapshot => ({
  node: runtime({ version: "24.0.0" }),
  // The runtimes endpoint only needs download state; names and account fields
  // come from /api/setup/status.
  providers: [{ id: "opencode", name: "opencode", runtime: runtime() }],
  ...over,
});

function setup(api: Record<string, (...a: never[]) => unknown>) {
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return useAssistantStore();
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("assistant store", () => {
  it("loads and merges the status with the runtime snapshot", async () => {
    const store = setup({
      setupStatus: vi.fn(async () =>
        status({ providers: [provider("opencode", { name: "OpenCode", runtime: runtime({ version: "1.0.0" }) })] }),
      ),
      setupRuntimes: vi.fn(async () =>
        snapshot({
          providers: [
            provider("opencode", { name: "OpenCode", runtime: runtime({ phase: "downloading", progress: 37 }) }),
          ],
        }),
      ),
    });

    await store.load();

    expect(store.loading).toBe(false);
    expect(store.error).toBeNull();
    expect(store.node).toMatchObject({ phase: "ready", version: "24.0.0" });
    expect(store.providers[0]).toMatchObject({
      id: "opencode",
      name: "OpenCode",
      runtime: { phase: "downloading", progress: 37 },
    });
  });

  it("keeps the status when an older daemon has no runtime snapshot", async () => {
    const store = setup({
      setupStatus: vi.fn(async () => status()),
      setupRuntimes: vi.fn(async () => {
        throw new Error("not found");
      }),
    });

    await store.load();

    expect(store.error).toBeNull();
    expect(store.providers).toHaveLength(1);
    expect(store.providers[0]!.runtime).toMatchObject({ phase: "ready", version: "2.2.5" });
  });

  it("records a load failure", async () => {
    const store = setup({
      setupStatus: vi.fn(async () => {
        throw new Error("boom");
      }),
      setupRuntimes: vi.fn(async () => snapshot()),
    });

    await store.load();

    expect(store.error).toBe("boom");
    expect(store.snapshot).toBeNull();
    expect(store.loading).toBe(false);
  });

  it("starts a download optimistically, then merges the daemon's answer", async () => {
    const answer = deferred<RuntimeStatus>();
    const downloadRuntime = vi.fn(() => answer.promise);
    const store = setup({
      setupStatus: vi.fn(async () =>
        status({ providers: [provider("opencode", { runtime: runtime({ phase: "notDownloaded" }) })] }),
      ),
      setupRuntimes: vi.fn(async () => snapshot()),
      downloadRuntime,
    });
    await store.load();

    const pending = store.download("opencode");
    expect(store.providers[0]!.runtime).toMatchObject({
      phase: "downloading",
      progress: 0,
      message: null,
    });
    expect(downloadRuntime).toHaveBeenCalledWith("opencode");

    answer.resolve(runtime({ phase: "downloading", progress: 12 }));
    await pending;
    expect(store.providers[0]!.runtime).toMatchObject({ phase: "downloading", progress: 12 });
  });

  it("cancels a download and restores the previous ready copy", async () => {
    const cancelRuntime = vi.fn(async () => runtime({ phase: "ready", version: "2.2.5" }));
    const store = setup({
      setupStatus: vi.fn(async () =>
        status({
          providers: [
            provider("opencode", { runtime: runtime({ phase: "downloading", progress: 55 }) }),
          ],
        }),
      ),
      setupRuntimes: vi.fn(async () => snapshot()),
      cancelRuntime,
    });
    await store.load();

    await store.cancel("opencode");

    expect(cancelRuntime).toHaveBeenCalledWith("opencode");
    expect(store.providers[0]!.runtime).toMatchObject({
      phase: "ready",
      version: "2.2.5",
      progress: null,
    });
  });

  it("cancels a first download back to not downloaded", async () => {
    const store = setup({
      setupStatus: vi.fn(async () =>
        status({
          providers: [
            provider("opencode", {
              runtime: runtime({ phase: "downloading", version: null, progress: 10 }),
            }),
          ],
        }),
      ),
      setupRuntimes: vi.fn(async () => snapshot()),
      cancelRuntime: vi.fn(async () =>
        runtime({ phase: "notDownloaded", version: null, source: null }),
      ),
    });
    await store.load();

    await store.cancel("opencode");

    expect(store.providers[0]!.runtime).toMatchObject({
      phase: "notDownloaded",
      version: null,
      progress: null,
    });
  });

  it("retries a failed download", async () => {
    const retryRuntime = vi.fn(async () => runtime({ phase: "downloading", progress: 1 }));
    const store = setup({
      setupStatus: vi.fn(async () =>
        status({
          providers: [
            provider("opencode", { runtime: runtime({ phase: "downloadError", message: "offline" }) }),
          ],
        }),
      ),
      setupRuntimes: vi.fn(async () => snapshot()),
      retryRuntime,
    });
    await store.load();

    await store.retry("opencode");

    expect(retryRuntime).toHaveBeenCalledWith("opencode");
    expect(store.providers[0]!.runtime).toMatchObject({ phase: "downloading", progress: 1 });
  });

  it("turns a failed retry into Download failed with its message", async () => {
    const store = setup({
      setupStatus: vi.fn(async () =>
        status({
          providers: [
            provider("opencode", { runtime: runtime({ phase: "downloadError", message: "offline" }) }),
          ],
        }),
      ),
      setupRuntimes: vi.fn(async () => snapshot()),
      retryRuntime: vi.fn(async () => {
        throw new Error("network down");
      }),
    });
    await store.load();

    await store.retry("opencode");

    expect(store.providers[0]!.runtime).toMatchObject({
      phase: "downloadError",
      progress: null,
      message: "network down",
    });
  });

  it("merges setup.runtime events into the snapshot", async () => {
    const store = setup({
      setupStatus: vi.fn(async () => status()),
      setupRuntimes: vi.fn(async () => snapshot()),
    });
    await store.load();

    h.daemon.emit({ type: "setup.runtime", runtime: "opencode", phase: "downloading", progress: 64 });
    expect(store.providers[0]!.runtime).toMatchObject({ phase: "downloading", progress: 64 });

    h.daemon.emit({ type: "setup.runtime", runtime: "opencode", phase: "finishing", progress: null });
    expect(store.providers[0]!.runtime).toMatchObject({ phase: "finishing", progress: null });

    h.daemon.emit({
      type: "setup.runtime",
      runtime: "opencode",
      phase: "ready",
      version: "2.2.6",
      source: "managed",
      progress: null,
    });
    expect(store.providers[0]!.runtime).toMatchObject({
      phase: "ready",
      version: "2.2.6",
      source: "managed",
    });

    h.daemon.emit({
      type: "setup.runtime",
      runtime: "node",
      phase: "ready",
      version: "24.1.0",
      source: "managed",
    });
    expect(store.node).toMatchObject({ phase: "ready", version: "24.1.0" });
  });

  it("ignores events for an unknown runtime", async () => {
    const store = setup({
      setupStatus: vi.fn(async () => status()),
      setupRuntimes: vi.fn(async () => snapshot()),
    });
    await store.load();
    const before = store.snapshot;

    h.daemon.emit({ type: "setup.runtime", runtime: "acp", phase: "downloading", progress: 5 });

    expect(store.snapshot).toBe(before);
  });

  it("filters chat-ready providers and recommends OpenCode", async () => {
    const store = setup({
      setupStatus: vi.fn(async () =>
        status({
          providers: [
            provider("cursor", { chatReady: false }),
            provider("claude", { chatReady: true }),
            provider("opencode", { chatReady: true, freeModels: true, signedIn: null }),
          ],
        }),
      ),
      setupRuntimes: vi.fn(async () => snapshot({ providers: [] })),
    });
    await store.load();

    expect(store.chatReadyProviders.map((p) => p.id)).toEqual(["opencode", "claude"]);
    expect(store.recommended?.id).toBe("opencode");
    expect(store.rows.map((r) => r.id)).toEqual(["opencode", "claude", "cursor"]);
  });

  it("verifies an assistant", async () => {
    const verifyProvider = vi.fn(async () => ({ chatReady: true, reason: null }));
    const store = setup({
      setupStatus: vi.fn(async () => status()),
      setupRuntimes: vi.fn(async () => snapshot()),
      verifyProvider,
    });
    await store.load();

    await store.verify("opencode");

    expect(verifyProvider).toHaveBeenCalledWith("opencode");
    expect(store.providers[0]).toMatchObject({ chatReady: true, reason: null });
  });

  it("runs the sign-in flow and marks a connected provider signed in", async () => {
    const signInProvider = vi.fn(async (): Promise<ProviderSignInState> => ({
      state: "browser",
      url: "https://example.test/auth",
    }));
    const submitProviderSignInCode = vi.fn(
      async (): Promise<ProviderSignInState> => ({ state: "connected" }),
    );
    const store = setup({
      setupStatus: vi.fn(async () =>
        status({ providers: [provider("claude", { signedIn: false, chatReady: false })] }),
      ),
      setupRuntimes: vi.fn(async () => snapshot()),
      signInProvider,
      submitProviderSignInCode,
    });
    await store.load();

    await store.signIn("claude");
    expect(store.signInFor("claude")).toMatchObject({
      state: "browser",
      url: "https://example.test/auth",
    });

    await store.submitCode("claude", "ABCD-1234");
    expect(submitProviderSignInCode).toHaveBeenCalledWith("claude", "ABCD-1234");
    expect(store.signInFor("claude")).toMatchObject({ state: "connected" });
    expect(store.providers[0]).toMatchObject({ signedIn: true });
  });

  it("cancels a sign-in and clears it", async () => {
    const store = setup({
      setupStatus: vi.fn(async () => status()),
      setupRuntimes: vi.fn(async () => snapshot()),
      signInProvider: vi.fn(async () => ({ state: "browser", url: "https://example.test" })),
      cancelProviderSignIn: vi.fn(async () => ({ state: "cancelled" as const })),
    });
    await store.load();
    await store.signIn("opencode");

    await store.cancelSignIn("opencode");
    expect(store.signInFor("opencode")).toMatchObject({ state: "cancelled" });

    store.clearSignIn("opencode");
    expect(store.signInFor("opencode")).toBeNull();
  });

  it("signs out and keeps OpenCode usable on free models", async () => {
    const store = setup({
      setupStatus: vi.fn(async () =>
        status({
          providers: [provider("opencode", { signedIn: true, chatReady: true, freeModels: true })],
        }),
      ),
      setupRuntimes: vi.fn(async () => snapshot()),
      signOutProvider: vi.fn(async () => ({ signedOut: true as const })),
    });
    await store.load();

    await store.signOut("opencode");

    expect(store.providers[0]).toMatchObject({ signedIn: false, chatReady: true });
  });

  it("disables chat after signing out of an account-only assistant", async () => {
    const store = setup({
      setupStatus: vi.fn(async () =>
        status({ providers: [provider("claude", { signedIn: true, chatReady: true })] }),
      ),
      setupRuntimes: vi.fn(async () => snapshot()),
      signOutProvider: vi.fn(async () => ({ signedOut: true as const })),
    });
    await store.load();

    await store.signOut("claude");

    expect(store.providers[0]).toMatchObject({ signedIn: false, chatReady: false });
  });

  it("reloads after reconnect and after events.lagged", async () => {
    const setupStatus = vi.fn(async () => status());
    const store = setup({
      setupStatus,
      setupRuntimes: vi.fn(async () => snapshot()),
    });
    await store.load();
    expect(setupStatus).toHaveBeenCalledTimes(1);

    h.daemon.simulateReconnect();
    await vi.waitFor(() => expect(setupStatus).toHaveBeenCalledTimes(2));

    h.daemon.emit({ type: "events.lagged", missed: 3 });
    await vi.waitFor(() => expect(setupStatus).toHaveBeenCalledTimes(3));
  });

  it("unsubscribes from events when the store is disposed", async () => {
    const store = setup({
      setupStatus: vi.fn(async () => status()),
      setupRuntimes: vi.fn(async () => snapshot()),
    });
    await store.load();
    expect(h.daemon.subscriberCount()).toBeGreaterThan(0);

    store.$dispose();

    expect(h.daemon.subscriberCount()).toBe(0);
  });
});
