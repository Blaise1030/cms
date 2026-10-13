// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { toast } from "vue-sonner";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { info: vi.fn(), success: vi.fn(), warning: vi.fn(), error: vi.fn() } }));

import GetLatestButton from "./GetLatestButton.vue";
import { useChangesStore } from "./store";
import { useAgentOutboxStore } from "@/modules/terminal/outbox";

function mountButton() {
  const updateChanges = vi.fn();
  h.daemon = createFakeDaemon({ updateChanges });
  setActivePinia(createPinia());
  return { w: mount(GetLatestButton, { props: { projectId: "site" } }), updateChanges };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("GetLatestButton", () => {
  it("asks the agent to bring in the latest main instead of syncing itself", async () => {
    const { w, updateChanges } = mountButton();
    await w.find("[data-testid='get-latest']").trigger("click");
    await flushPromises();
    expect(useAgentOutboxStore().messages).toEqual([
      {
        kind: "text",
        text: "Bring the latest version of the site (main) into my current change.",
        submit: true,
      },
    ]);
    expect(updateChanges).not.toHaveBeenCalled();
    expect(toast.info).toHaveBeenCalledWith("Asked the agent to get the latest");
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
});
