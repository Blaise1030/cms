// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h, ref, type PropType } from "vue";
import { TooltipProvider } from "reka-ui";
import BrowserFrame, { type BrowserFrameSide } from "./BrowserFrame.vue";
import { frameUrl, isHttpUrl, normalizePath } from "./frameUrl";

// Every measured pane reports this size (happy-dom lays nothing out); 0 = unmeasured.
const size = vi.hoisted(() => ({ width: 0, height: 0 }));
vi.mock("@vueuse/core", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@vueuse/core")>();
  const { ref } = await import("vue");
  return { ...actual, useElementSize: () => ({ width: ref(size.width), height: ref(size.height), stop: () => {} }) };
});

// The size menu's items as plain buttons, so a test can pick one without a portal.
vi.mock("@nuxt/ui/components/DropdownMenu.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    __esModule: true,
    default: defineComponent({
      props: { items: { type: Array as PropType<Record<string, unknown>[]>, required: true } },
      setup: (p, { slots }) => () =>
        h("div", [
          slots.default?.(),
          ...p.items.map((item) =>
            h(
              "button",
              { "data-testid": item.testid, onClick: () => (item.onUpdateChecked as () => void)() },
              String(item.label),
            ),
          ),
        ]),
    }),
  };
});

const CHANGE: BrowserFrameSide = { key: "change", label: "This change", url: "https://pr-7.site.test/" };
const LIVE: BrowserFrameSide = { key: "live", label: "Live site", url: "https://site.test/docs/" };

function mountFrame(frames: BrowserFrameSide[]) {
  const Host = defineComponent({
    setup: () => () =>
      h(TooltipProvider, null, {
        default: () =>
          h(BrowserFrame, { frames }, { placeholder: ({ frame }: { frame: BrowserFrameSide }) => h("p", { "data-testid": `placeholder-${frame.key}` }, "nothing") }),
      }),
  });
  return mount(Host, { attachTo: document.body });
}

const src = (w: ReturnType<typeof mountFrame>, key: string) =>
  w.find(`iframe[data-testid='browser-frame-iframe-${key}']`).attributes("src");

async function go(w: ReturnType<typeof mountFrame>, path: string) {
  await w.get("[data-testid='browser-frame-address']").setValue(path);
  await w.get("[data-testid='browser-frame-form']").trigger("submit");
  await flushPromises();
}

afterEach(() => {
  size.width = 0;
  size.height = 0;
  document.body.innerHTML = "";
  vi.restoreAllMocks();
});

describe("frameUrl", () => {
  it("opens the path on the base's site, under its prefix", () => {
    expect(frameUrl("https://site.test/", "/pricing")).toBe("https://site.test/pricing");
    expect(frameUrl("https://site.test/docs/", "/a?b=1#c")).toBe("https://site.test/docs/a?b=1#c");
    expect(frameUrl("https://site.test/?ref=x", "/")).toBe("https://site.test/");
  });

  it("refuses anything but http(s)", () => {
    expect(frameUrl("javascript:alert(1)", "/")).toBeNull();
    expect(frameUrl("data:text/html,hi", "/")).toBeNull();
    expect(frameUrl("/relative/", "/")).toBeNull();
    expect(frameUrl(null, "/")).toBeNull();
    expect(isHttpUrl("ftp://site.test/")).toBe(false);
    expect(isHttpUrl("http://site.test/")).toBe(true);
  });

  it("keeps only the path of what is typed", () => {
    expect(normalizePath("pricing")).toBe("/pricing");
    expect(normalizePath("  ")).toBe("/");
    expect(normalizePath("https://other.test/blog?p=2")).toBe("/blog?p=2");
  });
});

describe("BrowserFrame", () => {
  it("opens a typed path on every side", async () => {
    const w = mountFrame([CHANGE, LIVE]);
    expect(src(w, "change")).toBe("https://pr-7.site.test/");
    expect(src(w, "live")).toBe("https://site.test/docs/");
    await go(w, "/pricing");
    expect(src(w, "change")).toBe("https://pr-7.site.test/pricing");
    expect(src(w, "live")).toBe("https://site.test/docs/pricing");
    const frame = w.get("iframe[data-testid='browser-frame-iframe-change']");
    expect(frame.attributes("sandbox")).toBe("allow-scripts allow-same-origin allow-forms allow-popups");
    expect(frame.attributes("referrerpolicy")).toBe("no-referrer");
  });

  it("goes back and forward over the paths opened", async () => {
    const w = mountFrame([CHANGE]);
    const back = () => w.get("[data-testid='browser-frame-back']");
    const forward = () => w.get("[data-testid='browser-frame-forward']");
    expect(back().attributes("disabled")).toBeDefined();
    await go(w, "/a");
    await go(w, "/b");
    await back().trigger("click");
    expect(src(w, "change")).toBe("https://pr-7.site.test/a");
    await back().trigger("click");
    expect(src(w, "change")).toBe("https://pr-7.site.test/");
    await forward().trigger("click");
    expect(src(w, "change")).toBe("https://pr-7.site.test/a");
    // Opening a new path drops the forward entries.
    await go(w, "/c");
    expect(forward().attributes("disabled")).toBeDefined();
  });

  it("keeps the path when sides come and go", async () => {
    const frames = ref<BrowserFrameSide[]>([CHANGE]);
    const Host = defineComponent({
      setup: () => () =>
        h(TooltipProvider, null, { default: () => h(BrowserFrame, { frames: frames.value }) }),
    });
    const w = mount(Host, { attachTo: document.body });
    await go(w as unknown as ReturnType<typeof mountFrame>, "/pricing");
    frames.value = [CHANGE, LIVE];
    await flushPromises();
    expect(src(w as unknown as ReturnType<typeof mountFrame>, "live")).toBe("https://site.test/docs/pricing");
  });

  it("folds a side to a strip with its state and lets the other fill the width", async () => {
    const collapsed = ref<string | null>("live");
    const frames = [CHANGE, { ...LIVE, status: "Not found" }];
    const Host = defineComponent({
      setup: () => () =>
        h(TooltipProvider, null, {
          default: () =>
            h(BrowserFrame, {
              frames,
              collapsed: collapsed.value,
              "onUpdate:collapsed": (v: string | null) => (collapsed.value = v),
            }),
        }),
    });
    const w = mount(Host, { attachTo: document.body });
    const sideShown = (key: string) =>
      (w.get(`[data-testid='browser-frame-side-${key}']`).element as HTMLElement).style.display !== "none";
    expect(sideShown("live")).toBe(false);
    expect(sideShown("change")).toBe(true);
    expect(w.get("[data-testid='browser-frame-strip-live']").text()).toContain("Live site");
    expect(w.get("[data-testid='browser-frame-status-live']").text()).toBe("Not found");
    await w.get("[data-testid='browser-frame-collapse-change']").trigger("click");
    await flushPromises();
    expect(collapsed.value).toBe("change");
    expect(sideShown("live")).toBe(true);
    expect(sideShown("change")).toBe(false);
    await w.get("[data-testid='browser-frame-expand-change']").trigger("click");
    await flushPromises();
    expect(collapsed.value).toBeNull();
    expect(w.find("[data-testid^='browser-frame-strip-']").exists()).toBe(false);
  });

  it("reloads every side", async () => {
    const w = mountFrame([CHANGE, LIVE]);
    const before = [
      w.get("iframe[data-testid='browser-frame-iframe-change']").element,
      w.get("iframe[data-testid='browser-frame-iframe-live']").element,
    ];
    await w.get("[data-testid='browser-frame-reload']").trigger("click");
    await flushPromises();
    expect(w.get("iframe[data-testid='browser-frame-iframe-change']").element).not.toBe(before[0]);
    expect(w.get("iframe[data-testid='browser-frame-iframe-live']").element).not.toBe(before[1]);
  });

  it("lays desktop out at 1440 px and scales it to fill each side's own width", async () => {
    // Side by side on a 1440 px screen: each side is 720 px wide.
    size.width = 720;
    size.height = 900;
    const w = mountFrame([CHANGE, LIVE]);
    await flushPromises();
    for (const key of ["change", "live"]) {
      const frame = w.get(`iframe[data-testid='browser-frame-iframe-${key}']`).element as HTMLElement;
      expect(frame.style.width).toBe("1440px");
      expect(frame.style.transform).toBe("scale(0.5)");
      // Scaled back down to the pane's height, so it fills the pane without scrolling.
      expect(frame.style.height).toBe("1800px");
      const canvas = w.get(`[data-testid='browser-frame-iframe-${key}-canvas']`).element as HTMLElement;
      expect(canvas.style.width).toBe("720px");
    }
    const sizeButton = w.get("[data-testid='browser-frame-size']");
    expect(sizeButton.text()).toContain("50%");
    expect(sizeButton.attributes("aria-label")).toBe("Viewport: Desktop, 50%");
  });

  it("does not scale a side wide enough for desktop", async () => {
    size.width = 1600;
    size.height = 900;
    const w = mountFrame([CHANGE]);
    await flushPromises();
    const frame = w.get("iframe[data-testid='browser-frame-iframe-change']").element as HTMLElement;
    expect(frame.style.width).toBe("100%");
    expect(frame.style.transform).toBe("");
    expect(w.get("[data-testid='browser-frame-size']").text()).not.toContain("%");
  });

  it("lays every side out at the chosen device width", async () => {
    const w = mountFrame([CHANGE, LIVE]);
    await w.get("[data-testid='browser-frame-device-mobile']").trigger("click");
    await flushPromises();
    for (const key of ["change", "live"]) {
      const frame = w.get(`iframe[data-testid='browser-frame-iframe-${key}']`).element as HTMLElement;
      expect(frame.style.width).toBe("390px");
    }
    expect(w.get("[data-testid='browser-frame-size']").attributes("aria-label")).toContain("Mobile");
    await w.get("[data-testid='browser-frame-device-tablet']").trigger("click");
    await flushPromises();
    expect((w.get("iframe[data-testid='browser-frame-iframe-live']").element as HTMLElement).style.width).toBe("768px");
  });

  it("shows the placeholder instead of a non-http site", () => {
    const w = mountFrame([{ ...CHANGE, url: "javascript:alert(1)" }, { ...LIVE, url: null }]);
    expect(w.find("iframe").exists()).toBe(false);
    expect(w.find("[data-testid='placeholder-change']").exists()).toBe(true);
    expect(w.find("[data-testid='placeholder-live']").exists()).toBe(true);
    for (const b of w.findAll("[data-testid='browser-frame-open']")) expect(b.attributes("disabled")).toBeDefined();
  });

  it("opens the current page of a side in a new tab", async () => {
    const open = vi.spyOn(window, "open").mockReturnValue(null);
    const w = mountFrame([CHANGE, LIVE]);
    await go(w, "/pricing");
    const buttons = w.findAll("[data-testid='browser-frame-open']");
    expect(buttons).toHaveLength(2);
    await buttons[1]!.trigger("click");
    expect(open).toHaveBeenCalledWith("https://site.test/docs/pricing", "_blank", "noopener,noreferrer");
  });

  it("puts a single side's open button in the toolbar", async () => {
    const open = vi.spyOn(window, "open").mockReturnValue(null);
    const w = mountFrame([CHANGE]);
    const buttons = w.findAll("[data-testid='browser-frame-open']");
    expect(buttons).toHaveLength(1);
    await buttons[0]!.trigger("click");
    expect(open).toHaveBeenCalledWith("https://pr-7.site.test/", "_blank", "noopener,noreferrer");
  });
});

describe("BrowserFrame pick mode", () => {
  function mountPickable() {
    const picking = ref<string | null>(null);
    const picks: unknown[] = [];
    const Host = defineComponent({
      setup: () => () =>
        h(TooltipProvider, null, {
          default: () =>
            h(BrowserFrame, {
              frames: [CHANGE, LIVE],
              pickable: true,
              picking: picking.value,
              "onUpdate:picking": (v: string | null) => (picking.value = v),
              onPick: (a: unknown) => picks.push(a),
            }),
        }),
    });
    return { w: mount(Host, { attachTo: document.body }), picking, picks };
  }

  it("dims one side for picking and turns a drag into an area of the page", async () => {
    const { w, picking, picks } = mountPickable();
    await w.get("[data-testid='browser-frame-pick-change']").trigger("click");
    expect(picking.value).toBe("change");
    const overlay = w.get("[data-testid='browser-frame-pick-overlay-change']");
    expect(w.find("[data-testid='browser-frame-pick-overlay-live']").exists()).toBe(false);
    await overlay.trigger("pointerdown", { clientX: 120, clientY: 90, pointerId: 1 });
    await overlay.trigger("pointermove", { clientX: 40, clientY: 30, pointerId: 1 });
    expect(w.find("[data-testid='browser-frame-pick-box']").exists()).toBe(true);
    await overlay.trigger("pointerup", { clientX: 40, clientY: 30, pointerId: 1 });
    await flushPromises();
    // The overlay leaves first, then the area is reported.
    expect(picking.value).toBeNull();
    expect(picks).toEqual([{ side: "change", path: "/", device: "desktop", rect: { x: 40, y: 30, width: 80, height: 60 } }]);
    w.unmount();
  });

  it("ignores a click-sized drag, and Esc stops picking", async () => {
    const { w, picking, picks } = mountPickable();
    await w.get("[data-testid='browser-frame-pick-live']").trigger("click");
    const overlay = w.get("[data-testid='browser-frame-pick-overlay-live']");
    await overlay.trigger("pointerdown", { clientX: 10, clientY: 10, pointerId: 1 });
    await overlay.trigger("pointerup", { clientX: 14, clientY: 30, pointerId: 1 });
    expect(picks).toEqual([]);
    expect(picking.value).toBe("live");
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await flushPromises();
    expect(picking.value).toBeNull();
    expect(w.find("[data-testid='browser-frame-pick-overlay-live']").exists()).toBe(false);
    w.unmount();
  });
});

describe("BrowserFrame preview bridge", () => {
  function mountWithBridge() {
    let exposed!: { canCapture: (side: string) => boolean; captureSide: (side: string, rect: { x: number; y: number; width: number; height: number }) => Promise<string> };
    const Host = defineComponent({
      setup: () => () =>
        h(TooltipProvider, null, {
          default: () =>
            h(BrowserFrame, {
              frames: [CHANGE, LIVE],
              ref: (r: unknown) => r && (exposed = r as typeof exposed),
            }),
        }),
    });
    const w = mount(Host, { attachTo: document.body });
    return { w, bridge: () => exposed };
  }
  /** happy-dom frames have no window; give one a stand-in that records what it was sent. */
  function fakeWindow(el: Element) {
    const win = { postMessage: vi.fn() };
    Object.defineProperty(el, "contentWindow", { configurable: true, value: win });
    return win;
  }
  const reply = (source: unknown, data: Record<string, unknown>) =>
    window.dispatchEvent(new MessageEvent("message", { data: { type: "orso:preview", ...data }, source: source as Window }));

  it("says hello on load and learns which side can capture itself", async () => {
    const { w, bridge } = mountWithBridge();
    const iframe = w.get("iframe[data-testid='browser-frame-iframe-change']");
    const win = fakeWindow(iframe.element);
    const posted = win.postMessage;
    await iframe.trigger("load");
    expect(posted).toHaveBeenCalledWith({ type: "orso:preview", op: "hello" }, "*");
    expect(bridge().canCapture("change")).toBe(false);
    // A message from some other window is ignored.
    reply(window, { op: "ready", capture: true });
    expect(bridge().canCapture("change")).toBe(false);
    reply(win, { op: "ready", capture: true });
    expect(bridge().canCapture("change")).toBe(true);
    expect(bridge().canCapture("live")).toBe(false);
    w.unmount();
  });

  it("asks the page for its own pixels in the frame's CSS px, allowing for zoom", async () => {
    const { w, bridge } = mountWithBridge();
    const el = w.get("iframe[data-testid='browser-frame-iframe-change']").element as HTMLIFrameElement;
    const win = fakeWindow(el);
    const posted = win.postMessage;
    reply(win, { op: "ready", capture: true });
    // Shown at half size: 1440 px of page in 720 px on screen, at (100, 50).
    el.getBoundingClientRect = () => DOMRect.fromRect({ x: 100, y: 50, width: 720, height: 450 });
    Object.defineProperty(el, "offsetWidth", { configurable: true, value: 1440 });
    const shot = bridge().captureSide("change", { x: 110, y: 60, width: 100, height: 40 });
    const sent = posted.mock.calls.at(-1)![0] as { op: string; id: number; rect: unknown };
    expect(sent.op).toBe("capture");
    expect(sent.rect).toEqual({ x: 20, y: 20, width: 200, height: 80 });
    reply(win, { op: "capture", id: sent.id, ok: true, dataUrl: "data:image/png;base64,AAA" });
    await expect(shot).resolves.toBe("data:image/png;base64,AAA");
    // A refusal comes back as an error.
    const refused = bridge().captureSide("change", { x: 110, y: 60, width: 100, height: 40 });
    const again = posted.mock.calls.at(-1)![0] as { id: number };
    reply(win, { op: "capture", id: again.id, ok: false, error: "The capture came out blank." });
    await expect(refused).rejects.toThrow("The capture came out blank.");
    // A side without the bridge doesn't ask at all.
    await expect(bridge().captureSide("live", { x: 0, y: 0, width: 10, height: 10 })).rejects.toThrow("No preview bridge.");
    w.unmount();
  });
});
