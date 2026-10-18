// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useAgentProvidersStore } from "./providers";

const KEY = "workbench.terminal.provider";
const api = {
  terminalProviders: vi.fn(async () => [
    { id: "claude", command: ["claude"] },
    { id: "codex", command: ["codex"] },
  ]),
};

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
});

describe("useAgentProvidersStore", () => {
  it("keeps the saved pick when the daemon still offers it", async () => {
    localStorage.setItem(KEY, "codex");
    const store = useAgentProvidersStore();
    await store.loadProviders();
    expect(store.providers.map((p) => p.id)).toEqual(["claude", "codex"]);
    expect(store.provider).toBe("codex");
  });

  it("falls back to the first provider when the saved one is gone", async () => {
    localStorage.setItem(KEY, "cursor");
    const store = useAgentProvidersStore();
    await store.loadProviders();
    expect(store.provider).toBe("claude");
  });

  it("remembers the user's pick", () => {
    const store = useAgentProvidersStore();
    store.setProvider("codex");
    expect(store.provider).toBe("codex");
    expect(useAgentProvidersStore().provider).toBe("codex");
    setActivePinia(createPinia());
    expect(useAgentProvidersStore().provider).toBe("codex");
  });

  it("keeps the current pick when the daemon can't list providers", async () => {
    api.terminalProviders.mockRejectedValueOnce(new Error("offline"));
    const store = useAgentProvidersStore();
    store.setProvider("codex");
    await store.loadProviders();
    expect(store.providers).toEqual([]);
    expect(store.provider).toBe("codex");
  });
});
