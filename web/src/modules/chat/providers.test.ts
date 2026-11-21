// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useAssistantStore } from "@/modules/assistant/store";
import { useAgentProvidersStore } from "./providers";

const KEY = "orso.chat.provider";
const LEGACY_KEY = "orso.terminal.provider";
const api = {
  chatProviders: vi.fn(async () => [
    { id: "claude", command: ["claude"], acp: ["claude"] },
    { id: "codex", command: ["codex"], acp: ["codex"] },
  ]),
  agentModels: vi.fn(async (provider: string) => [{ id: `${provider}-m1`, name: "Model 1" }]),
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

  it("keeps the pick saved by the old Terminal tab", async () => {
    localStorage.setItem(LEGACY_KEY, "codex");
    const store = useAgentProvidersStore();
    await store.loadProviders();
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
    api.chatProviders.mockRejectedValueOnce(new Error("offline"));
    const store = useAgentProvidersStore();
    store.setProvider("codex");
    await store.loadProviders();
    expect(store.providers).toEqual([]);
    expect(store.provider).toBe("codex");
  });

  it("loads each provider's models once", async () => {
    const store = useAgentProvidersStore();
    await Promise.all([store.loadModels("codex"), store.loadModels("codex")]);
    await store.loadModels("codex");
    expect(store.models.codex).toEqual([{ id: "codex-m1", name: "Model 1" }]);
    expect(api.agentModels).toHaveBeenCalledTimes(1);
  });

  it("offers no models when the daemon can't list them, and asks again later", async () => {
    api.agentModels.mockRejectedValueOnce(new Error("offline"));
    const store = useAgentProvidersStore();
    await store.loadModels("claude");
    expect(store.models.claude).toEqual([]);
    await store.loadModels("claude");
    expect(store.models.claude).toEqual([{ id: "claude-m1", name: "Model 1" }]);
  });

  it("remembers the model pick per provider", () => {
    const store = useAgentProvidersStore();
    store.setModel("codex", "gpt-5.2");
    store.setModel("claude", "opus");
    setActivePinia(createPinia());
    const fresh = useAgentProvidersStore();
    expect(fresh.modelPick("codex")).toBe("gpt-5.2");
    expect(fresh.modelPick("claude")).toBe("opus");
    expect(fresh.modelPick("cursor")).toBeNull();
    fresh.setModel("codex", null);
    expect(fresh.modelPick("codex")).toBeNull();
    setActivePinia(createPinia());
    expect(useAgentProvidersStore().modelPick("codex")).toBeNull();
  });

  describe("preferred", () => {
    const ready = (id: string, chatReady = true) => ({ id, name: id, chatReady });

    it("is the remembered pick while it is ready and no order is saved", async () => {
      localStorage.setItem(KEY, "codex");
      const store = useAgentProvidersStore();
      await store.loadProviders();
      expect(store.preferred).toBe("codex");
      useAssistantStore().snapshot = { node: null, providers: [ready("claude"), ready("codex")] };
      expect(store.preferred).toBe("codex");
    });

    it("is the first ready assistant in the saved order", async () => {
      localStorage.setItem(KEY, "codex");
      const store = useAgentProvidersStore();
      await store.loadProviders();
      const assistant = useAssistantStore();
      assistant.snapshot = {
        node: null,
        providers: [ready("opencode"), ready("claude", false), ready("codex"), ready("cursor")],
      };
      assistant.order = ["claude", "cursor", "codex"];
      // Claude needs a sign-in, so the next one in the order starts new chats.
      expect(store.ready).toEqual(["cursor", "codex", "opencode"]);
      expect(store.preferred).toBe("cursor");
    });

    it("orders the daemon's list before readiness is known", async () => {
      const store = useAgentProvidersStore();
      await store.loadProviders();
      useAssistantStore().order = ["codex"];
      expect(store.ready).toEqual(["codex", "claude"]);
      expect(store.preferred).toBe("codex");
    });

    it("is nothing when no assistant is ready", () => {
      const store = useAgentProvidersStore();
      const assistant = useAssistantStore();
      assistant.snapshot = { node: null, providers: [ready("claude", false)] };
      assistant.order = ["claude"];
      expect(store.preferred).toBeNull();
    });
  });
});
