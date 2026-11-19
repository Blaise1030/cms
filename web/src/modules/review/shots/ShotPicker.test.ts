// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount } from "@vue/test-utils";
import ShotPicker from "./ShotPicker.vue";
import { resetShotQueuesForTest, useShotQueue } from "./queue";

const cap = vi.hoisted(() => ({ start: vi.fn(), grab: vi.fn(), active: false }));
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
  return { ...actual, TabCapture };
});

const AREA = { side: "change", path: "/pricing", device: "mobile" as const, rect: { x: 10, y: 20, width: 100, height: 50 } };
const body = () => new DOMWrapper(document.body);

beforeEach(() => {
  resetShotQueuesForTest();
  cap.start.mockReset().mockResolvedValue(undefined);
  cap.grab.mockReset().mockResolvedValue({ dataUrl: "data:image/png;base64,AAA=", mime: "image/png" });
  vi.stubGlobal("requestAnimationFrame", (cb: () => void) => setTimeout(cb, 0));
});
afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

function mountPicker() {
  return mount(ShotPicker, { props: { projectId: "site", number: 7, picking: null, picked: null }, attachTo: document.body });
}

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
