// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { CaptureUnavailable, encode, TabCapture } from "./capture";
import { MAX_PNG_BYTES } from "./crop";

const fakeCanvas = (sizes: Record<string, number>) =>
  ({
    toBlob: (cb: (b: Blob | null) => void, type: string) =>
      cb(new Blob([new Uint8Array(sizes[type] ?? 1)], { type })),
  }) as unknown as HTMLCanvasElement;

afterEach(() => {
  Object.defineProperty(navigator, "mediaDevices", { configurable: true, value: undefined });
});

describe("encode", () => {
  it("keeps a PNG up to 1 MB", async () => {
    const shot = await encode(fakeCanvas({ "image/png": 500_000 }));
    expect(shot.mime).toBe("image/png");
    expect(shot.dataUrl.startsWith("data:image/png;base64,")).toBe(true);
  });

  it("falls back to JPEG when the PNG is over 1 MB", async () => {
    const shot = await encode(fakeCanvas({ "image/png": MAX_PNG_BYTES + 1, "image/jpeg": 300_000 }));
    expect(shot.mime).toBe("image/jpeg");
  });
});

describe("TabCapture", () => {
  it("says the browser can't when there is no getDisplayMedia", async () => {
    const err = await new TabCapture().start().catch((e) => e);
    expect(err).toBeInstanceOf(CaptureUnavailable);
    expect(err.reason).toBe("unsupported");
  });

  it("says sharing was refused when the person declines", async () => {
    const getDisplayMedia = vi.fn(async () => {
      throw Object.assign(new Error("denied"), { name: "NotAllowedError" });
    });
    Object.defineProperty(navigator, "mediaDevices", { configurable: true, value: { getDisplayMedia } });
    const err = await new TabCapture().start().catch((e) => e);
    expect(err.reason).toBe("denied");
    expect(err.message).toBe("Screen sharing was not allowed.");
    expect(getDisplayMedia).toHaveBeenCalledWith(
      expect.objectContaining({ video: { displaySurface: "browser" }, preferCurrentTab: true, selfBrowserSurface: "include" }),
    );
  });
});
