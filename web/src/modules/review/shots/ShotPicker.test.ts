// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount } from "@vue/test-utils";
import ShotPicker from "./ShotPicker.vue";
import { resetShotQueuesForTest, useShotQueue } from "./queue";

const cap = vi.hoisted(() => ({ start: vi.fn(), grab: vi.fn(), active: false }));
const desk = vi.hoisted(() => ({ available: false, capture: vi.fn() }));
vi.mock("./desktop", () => ({
  desktopCaptureAvailable: () => desk.available,
  desktopCapture: desk.capture,
}));
vi.mock("./capture", async (orig) => {
  const actual = await orig<typeof import("./capture")>();
  class TabCapture {
    get active() {
      return cap.active;
    }
    start = cap.start;
    grab = cap.grab;
    stop() {}
  }
  // Images from the preview or the desktop app pass through as they are.
  const fitShot = async (dataUrl: string) => ({ dataUrl, mime: "image/png" as const });
  return { ...actual, TabCapture, fitShot };
});

const AREA = { side: "change", path: "/pricing", device: "mobile" as const, rect: { x: 10, y: 20, width: 100, height: 50 } };
const body = () => new DOMWrapper(document.body);

beforeEach(() => {
  resetShotQueuesForTest();
  cap.start.mockReset().mockResolvedValue(undefined);
  cap.grab.mockReset().mockResolvedValue({ dataUrl: "data:image/png;base64,AAA=", mime: "image/png" });
  desk.available = false;
  desk.capture.mockReset();
  vi.stubGlobal("requestAnimationFrame", (cb: () => void) => setTimeout(cb, 0));
});
afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

function mountPicker(
  bridge: { canCapture: (side: string) => boolean; captureSide: (side: string, rect: unknown) => Promise<string> } | null = null,
) {
  return mount(ShotPicker, { props: { projectId: "site", number: 7, picking: null, picked: null, bridge }, attachTo: document.body });
}

/** Picks `AREA` and waits for the note popover; returns where its screenshot came from. */
async function pickArea(w: ReturnType<typeof mountPicker>, side = "change") {
  await w.setProps({ picking: side });
  await flushPromises();
  await w.setProps({ picking: null, picked: { ...AREA, side } });
  await vi.waitFor(() => expect(body().find("[data-testid='shot-note']").exists()).toBe(true));
  const note = body().get("[data-testid='shot-note']");
  return { source: note.attributes("data-source"), image: body().find("[data-testid='shot-note-image']") };
}

describe("ShotPicker capture order", () => {
  it("takes the screenshot from the preview itself, without asking to share the screen", async () => {
    const captureSide = vi.fn(async () => "data:image/png;base64,PREVIEW=");
    const w = mountPicker({ canCapture: (side) => side === "change", captureSide });
    const { source, image } = await pickArea(w);
    expect(cap.start).not.toHaveBeenCalled();
    expect(captureSide).toHaveBeenCalledWith("change", AREA.rect);
    expect(source).toBe("preview");
    expect(image.attributes("src")).toBe("data:image/png;base64,PREVIEW=");
    expect(cap.grab).not.toHaveBeenCalled();
  });

  it("uses the desktop app's snapshot when the preview can't capture itself", async () => {
    desk.available = true;
    desk.capture.mockResolvedValue("data:image/png;base64,DESKTOP=");
    const captureSide = vi.fn(async () => {
      throw new Error("The capture came out blank.");
    });
    const w = mountPicker({ canCapture: () => true, captureSide });
    const { source, image } = await pickArea(w);
    expect(cap.start).not.toHaveBeenCalled();
    expect(captureSide).toHaveBeenCalled();
    expect(desk.capture).toHaveBeenCalledWith(AREA.rect);
    expect(source).toBe("desktop");
    expect(image.attributes("src")).toBe("data:image/png;base64,DESKTOP=");
  });

  it("falls back to sharing the tab, asked for when picking starts, without a bridge or the desktop app", async () => {
    const w = mountPicker({ canCapture: () => false, captureSide: vi.fn() });
    const { source } = await pickArea(w, "live");
    expect(cap.start).toHaveBeenCalledTimes(1);
    expect(cap.grab).toHaveBeenCalledWith(AREA.rect);
    expect(source).toBe("tab");
  });

  it("falls back to the tab when the desktop snapshot fails too", async () => {
    desk.available = true;
    desk.capture.mockRejectedValue(new Error("no"));
    const w = mountPicker(null);
    const { source } = await pickArea(w);
    // No prompt at Pick time (the desktop app was expected to work), so the tab is asked now.
    expect(cap.start).not.toHaveBeenCalled();
    expect(cap.grab).toHaveBeenCalled();
    expect(source).toBe("tab");
  });
});

describe("ShotPicker", () => {
  it("shares the tab when picking starts, cuts out the picked area and queues it with a note", async () => {
    const w = mountPicker();
    await w.setProps({ picking: "change" });
    await flushPromises();
    expect(cap.start).toHaveBeenCalledTimes(1);
    await w.setProps({ picking: null, picked: AREA });
    await vi.waitFor(() => expect(body().find("[data-testid='shot-note']").exists()).toBe(true));
    expect(cap.grab).toHaveBeenCalledWith(AREA.rect);
    expect(body().get("[data-testid='shot-note-image']").attributes("src")).toBe("data:image/png;base64,AAA=");
    await body().get("[data-testid='shot-note-input']").setValue("Price is cut off");
    await body().get("[data-testid='shot-note']").trigger("submit");
    await flushPromises();
    const queued = useShotQueue(() => "site", () => 7).items.value;
    expect(queued).toHaveLength(1);
    expect(queued[0]).toMatchObject({
      side: "change",
      path: "/pricing",
      viewport: "mobile 390px",
      note: "Price is cut off",
      imageDataUrl: "data:image/png;base64,AAA=",
    });
    expect(w.emitted("done")).toHaveLength(1);
  });

  it("offers a note without a screenshot when sharing is refused", async () => {
    const { CaptureUnavailable } = await import("./capture");
    cap.start.mockRejectedValue(new CaptureUnavailable("Screen sharing was not allowed.", "denied"));
    const w = mountPicker();
    await w.setProps({ picking: "live" });
    await flushPromises();
    expect(w.get("[data-testid='shot-unavailable']").text()).toContain("Screen sharing was not allowed.");
    await w.get("[data-testid='shot-notes-only']").trigger("click");
    await w.setProps({ picking: null, picked: { ...AREA, side: "live", device: "desktop" as const } });
    await vi.waitFor(() => expect(body().find("[data-testid='shot-note-no-image']").exists()).toBe(true));
    expect(cap.grab).not.toHaveBeenCalled();
    await body().get("[data-testid='shot-note-input']").setValue("Old copy here");
    await body().get("[data-testid='shot-note']").trigger("submit");
    await flushPromises();
    expect(useShotQueue(() => "site", () => 7).items.value[0]).toMatchObject({
      side: "live",
      viewport: "desktop",
      note: "Old copy here",
      imageDataUrl: null,
    });
  });
});
