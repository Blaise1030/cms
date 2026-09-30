// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import PreviewPanel from "./PreviewPanel.vue";
import { usePreviewStore } from "./store";

// Not attached to the document on purpose: happy-dom logs an error for every attached iframe.
function mountPanel(devserver: "starting" | "ready" | "crashed" = "ready", api = {}) {
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return mount(PreviewPanel, { props: { projectId: "site-1", devserver } });
}

describe("PreviewPanel", () => {
  it("embeds the proxied preview URL", () => {
    const w = mountPanel();
    expect(w.find("iframe").attributes("src")).toBe("http://127.0.0.1:7800/preview/site-1/");
    w.unmount();
  });

  it("sizes the iframe per device toggle", async () => {
    const w = mountPanel();
    const store = usePreviewStore();
    const width = () => (w.find("iframe").element as HTMLIFrameElement).style.width;
    expect(width()).toBe("100%");
    store.setDevice("tablet");
    await flushPromises();
    expect(width()).toBe("768px");
    store.setDevice("mobile");
    await flushPromises();
    expect(width()).toBe("390px");
    w.unmount();
  });

  it("remounts the iframe on manual reload", async () => {
    const w = mountPanel();
    const before = w.find("iframe").element;
    await w.find("[aria-label='Reload preview']").trigger("click");
    await flushPromises();
    expect(w.find("iframe").element).not.toBe(before);
    w.unmount();
  });

  it("reflects devserver.status on the status dot and reloads when ready", async () => {
    const w = mountPanel("starting");
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("starting");
    expect(w.find("iframe").exists()).toBe(false);
    h.daemon.emit({ type: "devserver.status", projectId: "site-1", state: "ready", port: 4000 });
    await flushPromises();
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("ready");
    expect(w.find("iframe").exists()).toBe(true);
    w.unmount();
  });

  it("offers a restart when the dev server crashed", async () => {
    const restartDevserver = vi.fn(async () => ({}));
    const w = mountPanel("crashed", { restartDevserver });
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("crashed");
    expect(w.find("iframe").exists()).toBe(false);
    await w.find("[role='alert'] button").trigger("click");
    await flushPromises();
    expect(restartDevserver).toHaveBeenCalledWith("site-1");
    w.unmount();
  });

  it("ignores status events for other projects", async () => {
    const w = mountPanel("ready");
    h.daemon.emit({ type: "devserver.status", projectId: "other", state: "crashed" });
    await flushPromises();
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("ready");
    w.unmount();
  });
});
