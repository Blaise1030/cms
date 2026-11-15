// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { TooltipProvider } from "reka-ui";
import { defineComponent, h as render } from "vue";
import { createPinia, setActivePinia } from "pinia";
import { toast } from "@/lib/toast";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("@/lib/toast", () => ({ toast: { info: vi.fn(), success: vi.fn(), warning: vi.fn(), error: vi.fn() } }));

import GetLatestButton from "./GetLatestButton.vue";
import { useChangesStore } from "./store";
import { useAgentOutboxStore } from "@/modules/chat/outbox";

function mountButton() {
  const updateChanges = vi.fn();
  h.daemon = createFakeDaemon({ updateChanges });
  setActivePinia(createPinia());
  // UTooltip needs a TooltipProvider; `UApp` supplies one in the app.
  const Host = defineComponent(() => () => render(TooltipProvider, null, () => render(GetLatestButton, { projectId: "site" })));
  return { w: mount(Host), updateChanges };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("GetLatestButton", () => {
  it("is labelled for writers", () => {
    const { w } = mountButton();
    expect(w.find("[data-testid='get-latest']").attributes("aria-label")).toBe("Sync with live site");
  });

  it("asks the daemon to sync instead of the agent", async () => {
    const { w, updateChanges } = mountButton();
    await w.find("[data-testid='get-latest']").trigger("click");
    await flushPromises();
    expect(updateChanges).toHaveBeenCalledWith("site");
    expect(useAgentOutboxStore().messages).toEqual([]);
    expect(toast.info).not.toHaveBeenCalled();
  });

  it("is disabled while switching or creating a change", async () => {
    const { w } = mountButton();
    const store = useChangesStore();
    store.switching = true;
    await flushPromises();
    expect(w.find("[data-testid='get-latest']").attributes("disabled")).toBeDefined();
    store.switching = false;
    store.creating = true;
    await flushPromises();
    expect(w.find("[data-testid='get-latest']").attributes("disabled")).toBeDefined();
    store.creating = false;
    await flushPromises();
    expect(w.find("[data-testid='get-latest']").attributes("disabled")).toBeUndefined();
  });

  it("is disabled when there is nothing to sync", async () => {
    const { w } = mountButton();
    const store = useChangesStore();
    store.syncNeeded = false;
    await flushPromises();
    const button = w.find("[data-testid='get-latest']");
    expect(button.attributes("disabled")).toBeDefined();
    expect(button.attributes("aria-label")).toBe("Already up to date");
  });

  it("stays enabled while the sync state is unknown", async () => {
    const { w } = mountButton();
    const store = useChangesStore();
    store.syncNeeded = null;
    await flushPromises();
    expect(w.find("[data-testid='get-latest']").attributes("disabled")).toBeUndefined();
  });
});
