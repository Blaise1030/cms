// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { info: vi.fn(), error: vi.fn(), success: vi.fn() } }));

import NewChangeDialog from "./NewChangeDialog.vue";
import { useAgentOutboxStore } from "@/modules/chat/outbox";

describe("NewChangeDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setActivePinia(createPinia());
  });

  it("creates the change through the API instead of asking the agent", async () => {
    const createChange = vi.fn(async (_p: string, title: string) => ({
      branch: "content/launch-week",
      title,
    }));
    h.daemon = createFakeDaemon({
      createChange,
      listChanges: async () => [],
      activeChange: async () => null,
      tree: async () => ({ entries: [] }),
    });
    const w = mount(NewChangeDialog, {
      props: { projectId: "site", open: true },
      attachTo: document.body,
    });
    await flushPromises();

    const input = document.querySelector<HTMLInputElement>("[aria-label='Change title']")!;
    input.value = "Launch week";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await flushPromises();
    document.querySelector<HTMLFormElement>("[role='dialog'] form")!.requestSubmit();
    await flushPromises();

    expect(createChange).toHaveBeenCalledWith("site", "Launch week");
    expect(useAgentOutboxStore().messages).toEqual([]);
    expect(w.emitted("created")?.[0]?.[0]).toMatchObject({
      branch: "content/launch-week",
      title: "Launch week",
    });
    w.unmount();
  });
});
