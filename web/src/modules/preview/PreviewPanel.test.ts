// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const hDaemon = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => hDaemon.daemon }));

import PreviewPanel from "./PreviewPanel.vue";
import { usePreviewStore } from "./store";
import { useChangesStore } from "@/modules/changes/store";
import { useAgentOutboxStore } from "@/modules/chat/outbox";

// Not attached to the document on purpose: happy-dom logs an error for every attached iframe.
function mountPanel(devserver: "starting" | "ready" | "crashed" = "ready", api = {}) {
  localStorage.clear();
  hDaemon.daemon = createFakeDaemon(api);
  // Default to the embedded topology (page and daemon share an origin).
  hDaemon.daemon.config.value = { origin: window.location.origin };
  setActivePinia(createPinia());
  return mount(PreviewPanel, { props: { projectId: "site-1", devserver } });
}

describe("PreviewPanel", () => {
  it("leaves the PR status out of the browser bar (it lives behind the share button)", async () => {
    const w = mountPanel();
    useChangesStore().status = {
      number: 3,
      url: "https://github.com/acme/site/pull/3",
      state: "draft",
      checks: "passing",
      previewUrl: "https://lw.pages.dev",
    };
    await flushPromises();
    expect(w.find("[data-testid='pr-status']").exists()).toBe(false);
    expect(w.find("[data-testid='ready-for-review']").exists()).toBe(false);
    w.unmount();
  });

  it("embeds the proxied preview URL", () => {
    const w = mountPanel();
    expect(w.find("iframe").attributes("src")).toBe("http://127.0.0.1:7800/preview/site-1/");
    w.unmount();
  });

  it("opens the site's own dev server in a new tab, so a browser without the session can load it", async () => {
    localStorage.clear();
    hDaemon.daemon = createFakeDaemon({});
    hDaemon.daemon.config.value = { origin: window.location.origin };
    setActivePinia(createPinia());
    localStorage.setItem("orso.preview.path.site-1", "/blog/?x=1");
    const w = mount(PreviewPanel, { props: { projectId: "site-1", devserver: "ready", port: 56066 } });
    const open = vi.spyOn(window, "open").mockImplementation(() => null);
    await w.find("button[aria-label='Open in new tab']").trigger("click");
    expect(open).toHaveBeenCalledWith("http://127.0.0.1:56066/preview/site-1/blog/?x=1", "_blank", "noopener");
    open.mockRestore();
    w.unmount();
  });

  it("explains instead of framing a cross-site daemon, with a manual override", async () => {
    const w = mountPanel();
    hDaemon.daemon.config.value = { origin: "https://daemon.invalid:7800" };
    await flushPromises();
    expect(w.find("iframe").exists()).toBe(false);
    expect(w.find("[data-testid='preview-cross-site']").text()).toContain("daemon.invalid");
    await w.find("[data-testid='preview-cross-site'] button").trigger("click");
    expect(w.find("iframe").exists()).toBe(true);
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

  it("toggles RTL and dark mode for the frame", async () => {
    const w = mountPanel();
    const store = usePreviewStore();
    const iframe = () => w.find("iframe").element as HTMLIFrameElement;
    const trigger = () => w.find("[data-testid='preview-appearance']");
    expect(iframe().style.colorScheme).toBe("light");
    expect(trigger().attributes("data-variant")).toBe("ghost");
    store.toggleDark();
    await flushPromises();
    expect(iframe().style.colorScheme).toBe("dark");
    expect(trigger().attributes("data-variant")).toBe("secondary");
    expect(localStorage.getItem("orso.preview.dark")).toBe("true");
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

  it("shows the browser bar and navigates the frame from the address field", async () => {
    const w = mountPanel();
    expect(w.find("[aria-label='Back']").attributes("disabled")).toBeDefined();
    expect(w.find("[aria-label='Forward']").attributes("disabled")).toBeDefined();
    const address = w.find("[data-testid='preview-address']");
    expect((address.element as HTMLInputElement).value).toBe("/");
    await address.setValue("about/");
    await w.find("form").trigger("submit");
    expect(w.find("iframe").attributes("src")).toBe("http://127.0.0.1:7800/preview/site-1/about/");
    w.unmount();
  });

  it("reopens the last visited path for the project", async () => {
    const w = mountPanel();
    localStorage.setItem("orso.preview.path.site-1", "/pricing/?plan=pro");
    w.unmount();
    hDaemon.daemon = createFakeDaemon();
    hDaemon.daemon.config.value = { origin: window.location.origin };
    setActivePinia(createPinia());
    const again = mount(PreviewPanel, { props: { projectId: "site-1", devserver: "ready" } });
    expect(again.find("iframe").attributes("src")).toBe(
      "http://127.0.0.1:7800/preview/site-1/pricing/?plan=pro",
    );
    expect((again.find("[data-testid='preview-address']").element as HTMLInputElement).value).toBe(
      "/pricing/?plan=pro",
    );
    again.unmount();
  });

  it("reflects devserver.status on the status dot and reloads when ready", async () => {
    const w = mountPanel("starting");
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("starting");
    expect(w.find("iframe").exists()).toBe(false);
    hDaemon.daemon.emit({ type: "devserver.status", projectId: "site-1", state: "ready", port: 4000 });
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

  it("drops event-derived status and reloads the frame on events.lagged", async () => {
    const w = mountPanel("ready");
    hDaemon.daemon.emit({ type: "devserver.status", projectId: "site-1", state: "crashed" });
    await flushPromises();
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("crashed");
    // The crash was superseded by events we missed; the project list says ready.
    hDaemon.daemon.emit({ type: "events.lagged", missed: 4 });
    await flushPromises();
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("ready");
    expect(w.find("iframe").exists()).toBe(true);
    w.unmount();
  });

  it("remounts the iframe on preview.reload for this project", async () => {
    const w = mountPanel("ready");
    const before = usePreviewStore().reloadNonce;
    hDaemon.daemon.emit({ type: "preview.reload", projectId: "other" });
    await flushPromises();
    expect(usePreviewStore().reloadNonce).toBe(before);
    hDaemon.daemon.emit({ type: "preview.reload", projectId: "site-1" });
    await flushPromises();
    expect(usePreviewStore().reloadNonce).toBe(before + 1);
    expect(w.find("iframe").exists()).toBe(true);
    w.unmount();
  });

  it("offers to have the agent fix a crashed dev server", async () => {
    const w = mountPanel("ready");
    hDaemon.daemon.emit({
      type: "devserver.status",
      projectId: "site-1",
      state: "crashed",
      message: "Cannot find module 'astro'",
    });
    await flushPromises();
    await w.find("[data-testid='fix-crash']").trigger("click");
    const [message] = useAgentOutboxStore().messages;
    expect(message).toMatchObject({ kind: "text" });
    expect(message && "text" in message && message.text).toContain("Cannot find module 'astro'");
    expect(message && "submit" in message).toBe(false);
    w.unmount();
  });

  it("ignores status events for other projects", async () => {
    const w = mountPanel("ready");
    hDaemon.daemon.emit({ type: "devserver.status", projectId: "other", state: "crashed" });
    await flushPromises();
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("ready");
    w.unmount();
  });
});
