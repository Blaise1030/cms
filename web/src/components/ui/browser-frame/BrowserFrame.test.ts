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
