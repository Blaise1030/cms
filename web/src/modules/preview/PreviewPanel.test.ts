// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { TooltipProvider } from "reka-ui";
import UButton from "@nuxt/ui/components/Button.vue";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const hDaemon = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => hDaemon.daemon }));

import PreviewPanel from "./PreviewPanel.vue";
import { usePreviewStore } from "./store";
import { useChangesStore } from "@/modules/changes/store";
import { useAgentOutboxStore } from "@/modules/chat/outbox";
import { useChatPanelLayout } from "@/lib/chatPanelLayout";

/** The project's preview origin: the daemon's host (the test page's), its own port. */
const PORT = 56066;
const TOKEN = "pt";
const ORIGIN = () => `http://${window.location.hostname}:${PORT}`;

/** Tooltips need the provider `UApp` gives the real app. */
function mountPreview(options: { props: InstanceType<typeof PreviewPanel>["$props"]; attachTo?: Element }) {
  const Host = defineComponent({
    setup: () => () => h(TooltipProvider, null, { default: () => h(PreviewPanel, options.props) }),
  });
  return mount(Host, { attachTo: options.attachTo });
}

/**
 * Not attached to the document unless `attach` (happy-dom logs an error for every attached
 * iframe); attached frames get a `srcdoc` window the tests can post from and spy on.
 */
async function mountPanel(
  devserver: "starting" | "ready" | "crashed" = "ready",
  api: Record<string, unknown> = {},
  opts: { attach?: boolean; path?: string } = {},
) {
  localStorage.clear();
  if (opts.path) localStorage.setItem("orso.preview.path.site-1", opts.path);
  hDaemon.daemon = createFakeDaemon({ previewAccess: async () => ({ port: PORT, token: TOKEN }), ...api });
  // Default to the embedded topology (page and daemon share an origin).
  hDaemon.daemon.config.value = { origin: window.location.origin };
  setActivePinia(createPinia());
  const w = mountPreview({ props: { projectId: "site-1", devserver }, attachTo: opts.attach ? document.body : undefined });
  await flushPromises();
  return w;
}

type Wrapper = Awaited<ReturnType<typeof mountPanel>>;
const frameEl = (w: Wrapper) => w.find("iframe").element as HTMLIFrameElement;

/** Give an attached frame a window, and record what the panel posts to it. */
async function bridgeWindow(w: Wrapper) {
  frameEl(w).srcdoc = "<!doctype html><html><body></body></html>";
  const sent: { message: Record<string, unknown>; origin: string }[] = [];
  const win = frameEl(w).contentWindow!;
  win.postMessage = ((message: Record<string, unknown>, origin: string) => {
    sent.push({ message, origin });
  }) as typeof win.postMessage;
  await flushPromises();
  return { win, sent };
}

/** A bridge message from the frame (from the preview origin unless `origin` says otherwise). */
function fromFrame(w: Wrapper, data: Record<string, unknown>, origin = ORIGIN()) {
  window.dispatchEvent(
    new MessageEvent("message", { data: { type: "orso:preview", ...data }, source: frameEl(w).contentWindow, origin }),
  );
}

const address = (w: Wrapper) => (w.find("[data-testid='preview-address']").element as HTMLInputElement).value;

describe("PreviewPanel", () => {
  it("leaves the PR status out of the browser bar (it lives behind the share button)", async () => {
    const w = await mountPanel();
    useChangesStore().status = {
      number: 3,
      url: "https://github.com/acme/site/pull/3",
      state: "draft",
      checks: "passing",
      previewUrl: "https://lw.pages.dev",
      mergeable: null,
      mergeableState: "draft",
      upToDate: false,
    };
    await flushPromises();
    expect(w.find("[data-testid='pr-status']").exists()).toBe(false);
    expect(w.find("[data-testid='ready-for-review']").exists()).toBe(false);
    w.unmount();
  });

  it("frames the site at the root of its own origin, the first load carrying the token", async () => {
    const w = await mountPanel("ready", {}, { path: "/blog/" });
    expect(w.find("iframe").attributes("src")).toBe(`${ORIGIN()}/blog/?t=${TOKEN}`);
    expect(address(w)).toBe("/blog/");
    w.unmount();
  });

  it("frames without a token when the daemon runs without auth", async () => {
    const w = await mountPanel("ready", { previewAccess: async () => ({ port: PORT, token: null }) });
    expect(w.find("iframe").attributes("src")).toBe(`${ORIGIN()}/`);
    w.unmount();
  });

  it("says so when the preview origin can't be opened, and retries", async () => {
    const previewAccess = vi.fn().mockRejectedValueOnce(new Error("no port")).mockResolvedValue({ port: PORT, token: TOKEN });
    const w = await mountPanel("ready", { previewAccess });
    expect(w.find("iframe").exists()).toBe(false);
    expect(w.get("[data-testid='preview-opening']").text()).toContain("no port");
    await w.get("[data-testid='preview-opening'] button").trigger("click");
    await flushPromises();
    expect(w.find("iframe").exists()).toBe(true);
    w.unmount();
  });

  it("opens Page details as a resizable sidebar from the toolbar", async () => {
    const w = await mountPanel();
    expect(w.get("[data-testid='page-details-toggle']").attributes("aria-pressed")).toBe("false");
    expect(w.find("[data-testid='page-details-sidebar']").exists()).toBe(false);
    await w.get("[data-testid='page-details-toggle']").trigger("click");
    expect(w.get("[data-testid='page-details-toggle']").attributes("aria-pressed")).toBe("true");
    const sidebar = w.get("[data-testid='page-details-sidebar']");
    expect(sidebar.text()).toContain("Page details");
    expect(w.find("[data-slot='handle']").exists()).toBe(true);
    await w.get("[data-testid='page-details-close']").trigger("click");
    expect(w.find("[data-testid='page-details-sidebar']").exists()).toBe(false);
    w.unmount();
  });

  it("shows SEO fields in Page details from a bridge snapshot", async () => {
    const w = await mountPanel("ready", {}, { attach: true });
    const { sent } = await bridgeWindow(w);
    await w.get("[data-testid='page-details-toggle']").trigger("click");
    // The panel asks the page for its HTML, addressed to the preview origin only.
    expect(sent.some((s) => s.message.op === "snapshot" && s.origin === ORIGIN())).toBe(true);
    expect(sent.every((s) => s.origin === ORIGIN())).toBe(true);
    fromFrame(w, {
      op: "snapshot",
      url: `${ORIGIN()}/`,
      html: `<!doctype html><html lang="en"><head>
        <title>Welcome</title>
        <meta name="description" content="Hello world" />
        <link rel="icon" href="/favicon.svg" />
        <meta property="og:image" content="https://cdn.example/og.png" />
      </head><body><h1>Welcome</h1></body></html>`,
    });
    await flushPromises();
    await w.get("[data-testid='page-details-tab-meta']").trigger("click");
    await flushPromises();
    const meta = w.get("[data-testid='page-details-meta']");
    expect(meta.text()).toContain("Welcome");
    expect(meta.text()).toContain("Hello world");
    expect(meta.text()).toContain("/favicon.svg");
    expect(meta.text()).toContain("/og.png");
    // Images load from the preview origin (the frame's cookie authenticates them), no token.
    expect(w.get("[data-testid='page-details-favicon']").attributes("src")).toBe(`${ORIGIN()}/favicon.svg`);
    expect(w.get("[data-testid='page-details-og-image']").attributes("src")).toBe(`${ORIGIN()}/og.png`);
    w.unmount();
  });

  it("ignores bridge messages from any other origin", async () => {
    const w = await mountPanel("ready", {}, { attach: true });
    await bridgeWindow(w);
    fromFrame(w, { op: "path", url: "http://localhost:3999/evil/" }, "http://localhost:3999");
    fromFrame(w, { op: "error", error: { kind: "runtime", message: "spoofed" } }, "http://localhost:3999");
    await flushPromises();
    expect(address(w)).toBe("/");
    expect(w.find("[data-testid='page-error']").exists()).toBe(false);
    fromFrame(w, { op: "path", url: `${ORIGIN()}/about/` });
    await flushPromises();
    expect(address(w)).toBe("/about/");
    w.unmount();
  });

  it("introduces itself to the page on load and sends the appearance", async () => {
    const w = await mountPanel("ready", {}, { attach: true });
    const { sent } = await bridgeWindow(w);
    sent.length = 0;
    frameEl(w).dispatchEvent(new Event("load"));
    await flushPromises();
    expect(sent.map((s) => s.message.op)).toEqual(expect.arrayContaining(["hello", "appearance", "snapshot"]));
    expect(sent.every((s) => s.origin === ORIGIN())).toBe(true);
    w.unmount();
  });

  it("shows a page error the bridge reports and offers the agent a fix", async () => {
    const w = await mountPanel("ready", {}, { attach: true });
    await bridgeWindow(w);
    fromFrame(w, { op: "error", error: { kind: "build", message: "Unexpected token", file: "src/x.astro:1:2" } });
    await flushPromises();
    expect(w.get("[data-testid='page-error']").text()).toContain("Unexpected token");
    await w.get("[data-testid='fix-page-error']").trigger("click");
    const [message] = useAgentOutboxStore().messages;
    expect(message && "text" in message && message.text).toContain("src/x.astro:1:2");
    fromFrame(w, { op: "error", error: { kind: "build", message: "Again" } });
    await flushPromises();
    fromFrame(w, { op: "error-cleared" });
    await flushPromises();
    expect(w.find("[data-testid='page-error']").exists()).toBe(false);
    w.unmount();
  });

  it("uses a full radius when the agent sits in the bottom chrome", async () => {
    useChatPanelLayout().set("sidebar");
    const docked = await mountPanel();
    expect(docked.get("[data-testid='preview-shell']").classes()).toContain("rounded-t-xl");
    expect(docked.get("[data-testid='preview-shell']").classes()).toContain("border-b-0");
    docked.unmount();

    const floating = await mountPanel();
    useChatPanelLayout().set("floating");
    await flushPromises();
    expect(floating.get("[data-testid='preview-shell']").classes()).toContain("rounded-xl");
    expect(floating.get("[data-testid='preview-shell']").classes()).not.toContain("border-b-0");
    useChatPanelLayout().set("sidebar");
    floating.unmount();
  });

  it("reopens the page the frame was on when the view reloads", async () => {
    const w = await mountPanel();
    // The frame reports where the reader went (the injected bridge forwards it); the agent's
    // checkpoint then reloads the preview, which must come back to that page.
    usePreviewStore().setFramePath("site-1", "/blog/");
    await flushPromises();
    usePreviewStore().reload();
    await flushPromises();
    expect(w.find("iframe").attributes("src")).toBe(`${ORIGIN()}/blog/?t=${TOKEN}`);
    w.unmount();
  });

  it("remounts onto openPath even when frame history already has a page", async () => {
    const w = await mountPanel("ready", {}, { attach: true });
    await bridgeWindow(w);
    fromFrame(w, { op: "path", url: `${ORIGIN()}/about/` });
    try {
      await flushPromises();
      expect(address(w)).toBe("/about/");
      const before = w.find("iframe").element;
      usePreviewStore().openPath("site-1", "/blog/welcome/");
      await flushPromises();
      const after = w.find("iframe").element as HTMLIFrameElement;
      expect(after).not.toBe(before);
      expect(after.getAttribute("src")).toBe(`${ORIGIN()}/blog/welcome/?t=${TOKEN}`);
      expect(address(w)).toBe("/blog/welcome/");
      usePreviewStore().reload();
      await flushPromises();
      expect(w.find("iframe").attributes("src")).toBe(`${ORIGIN()}/blog/welcome/?t=${TOKEN}`);
    } finally {
      w.unmount();
    }
  });

  it("drives picking over the bridge and mentions the pick with its screenshot", async () => {
    const attachChatFile = vi.fn(async () => "/state/shot.png");
    const w = await mountPanel("ready", { attachChatFile }, { attach: true });
    const { sent } = await bridgeWindow(w);
    const store = usePreviewStore();
    expect(w.find("[data-testid='element-picker']").attributes("disabled")).toBeUndefined();
    await w.find("[data-testid='element-picker']").trigger("click");
    await flushPromises();
    expect(store.picking).toBe(true);
    expect(sent.at(-1)).toEqual({ message: { type: "orso:preview", op: "pick:start" }, origin: ORIGIN() });

    fromFrame(w, {
      op: "pick",
      element: {
        label: "h1",
        tag: "<h1>",
        selector: "h1",
        text: "Hi",
        file: "/home/me/orso-projects/site-1/src/pages/index.astro",
        screenshot: "data:image/png;base64,iVBORw0KGgo=",
      },
    });
    await flushPromises();
    expect(attachChatFile).toHaveBeenCalledWith("site-1", expect.any(Blob));
    const [message] = useAgentOutboxStore().messages;
    expect(message).toMatchObject({
      kind: "element",
      element: { label: "h1", file: "src/pages/index.astro", screenshot: "/state/shot.png" },
    });
    expect(store.picking).toBe(false);
    w.unmount();
  });

  it("cancels a selection when Escape is pressed in the app", async () => {
    const w = await mountPanel();
    const store = usePreviewStore();
    await w.find("[data-testid='element-picker']").trigger("click");
    await flushPromises();
    expect(store.picking).toBe(true);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await flushPromises();
    expect(store.picking).toBe(false);
    w.unmount();
  });

  it("navigates the frame from the address field", async () => {
    const w = await mountPanel();
    expect(w.find("[aria-label='Back']").attributes("disabled")).toBeDefined();
    expect(w.find("[aria-label='Forward']").attributes("disabled")).toBeDefined();
    expect(address(w)).toBe("/");
    await w.find("[data-testid='preview-address']").setValue("about/");
    await w.find("form").trigger("submit");
    expect(w.find("iframe").attributes("src")).toBe(`${ORIGIN()}/about/?t=${TOKEN}`);
    expect(address(w)).toBe("/about/");
    w.unmount();
  });

  it("opens the page on its preview origin in a new tab, with the token for a fresh browser", async () => {
    const w = await mountPanel("ready", {}, { path: "/blog/?x=1" });
    const open = vi.spyOn(window, "open").mockImplementation(() => null);
    await w.find("button[aria-label='Open in new tab']").trigger("click");
    expect(open).toHaveBeenCalledWith(`${ORIGIN()}/blog/?x=1&t=${TOKEN}`, "_blank", "noopener");
    open.mockRestore();
    w.unmount();
  });

  it("explains instead of framing a cross-site daemon, with a manual override", async () => {
    const w = await mountPanel();
    hDaemon.daemon.config.value = { origin: "https://daemon.invalid:7800" };
    await flushPromises();
    expect(w.find("iframe").exists()).toBe(false);
    expect(w.find("[data-testid='preview-cross-site']").text()).toContain("daemon.invalid");
    await w.find("[data-testid='preview-cross-site'] button").trigger("click");
    expect(w.find("iframe").exists()).toBe(true);
    // The preview origin follows the host the app reaches the daemon by.
    expect(w.find("iframe").attributes("src")).toBe(`http://daemon.invalid:${PORT}/?t=${TOKEN}`);
    w.unmount();
  });

  it("sizes the iframe per device toggle", async () => {
    const w = await mountPanel();
    const store = usePreviewStore();
    const width = () => frameEl(w).style.width;
    expect(width()).toBe("100%");
    store.setDevice("tablet");
    await flushPromises();
    expect(width()).toBe("768px");
    store.setDevice("mobile");
    await flushPromises();
    expect(width()).toBe("390px");
    w.unmount();
  });

  it("outlines a device-sized preview so its edge is visible", async () => {
    const w = await mountPanel();
    const store = usePreviewStore();
    const boundary = () => w.find("[data-testid='preview-boundary']");
    expect(boundary().exists()).toBe(false);
    store.setDevice("tablet");
    await flushPromises();
    expect(boundary().exists()).toBe(true);
    store.setDevice("desktop");
    await flushPromises();
    expect(boundary().exists()).toBe(false);
    w.unmount();
  });

  it("toggles RTL and dark mode for the frame", async () => {
    const w = await mountPanel("ready", {}, { attach: true });
    const { sent } = await bridgeWindow(w);
    const store = usePreviewStore();
    const trigger = () =>
      w.findAllComponents(UButton).find((b) => b.attributes("data-testid") === "preview-appearance")!;
    expect(frameEl(w).style.colorScheme).toBe("light");
    expect(trigger().props("variant")).toBe("ghost");
    store.toggleDark();
    await flushPromises();
    expect(frameEl(w).style.colorScheme).toBe("dark");
    expect(trigger().props("variant")).toBe("soft");
    expect(localStorage.getItem("orso.preview.dark")).toBe("true");
    expect(sent.at(-1)).toEqual({
      message: { type: "orso:preview", op: "appearance", rtl: false, dark: true },
      origin: ORIGIN(),
    });
    w.unmount();
  });

  it("remounts the iframe on manual reload", async () => {
    const w = await mountPanel();
    const before = w.find("iframe").element;
    await w.find("[aria-label='Reload preview']").trigger("click");
    await flushPromises();
    expect(w.find("iframe").element).not.toBe(before);
    w.unmount();
  });

  it("reflects devserver.status on the status dot and shows the frame when ready", async () => {
    const w = await mountPanel("starting");
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("starting");
    expect(w.find("iframe").exists()).toBe(false);
    expect(w.find("[data-testid='preview-starting'] [aria-label='Loading']").exists()).toBe(true);
    hDaemon.daemon.emit({ type: "devserver.status", projectId: "site-1", state: "ready", port: 4000 });
    await flushPromises();
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("ready");
    expect(w.find("iframe").exists()).toBe(true);
    w.unmount();
  });

  it("offers a restart when the dev server crashed", async () => {
    const restartDevserver = vi.fn(async () => ({}));
    const w = await mountPanel("crashed", { restartDevserver });
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("crashed");
    expect(w.find("iframe").exists()).toBe(false);
    await w.find("[role='alert'] button").trigger("click");
    await flushPromises();
    expect(restartDevserver).toHaveBeenCalledWith("site-1");
    w.unmount();
  });

  it("drops event-derived status and reloads the frame on events.lagged", async () => {
    const w = await mountPanel("ready");
    hDaemon.daemon.emit({ type: "devserver.status", projectId: "site-1", state: "crashed" });
    await flushPromises();
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("crashed");
    hDaemon.daemon.emit({ type: "events.lagged", missed: 4 });
    await flushPromises();
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("ready");
    expect(w.find("iframe").exists()).toBe(true);
    w.unmount();
  });

  it("remounts the iframe on preview.reload for this project", async () => {
    const w = await mountPanel("ready");
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
    const w = await mountPanel("ready");
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
    const w = await mountPanel("ready");
    hDaemon.daemon.emit({ type: "devserver.status", projectId: "other", state: "crashed" });
    await flushPromises();
    expect(w.find("[data-testid='status-dot']").attributes("data-state")).toBe("ready");
    w.unmount();
  });
});
