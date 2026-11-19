// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { desktopCapture, desktopCaptureAvailable } from "./desktop";

const rect = { x: 10, y: 20, width: 300, height: 200 };

afterEach(() => {
  delete document.documentElement.dataset.desktop;
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});

describe("desktop capture", () => {
  it("is only there in the macOS desktop app", () => {
    expect(desktopCaptureAvailable()).toBe(false);
    document.documentElement.dataset.desktop = "macos";
    expect(desktopCaptureAvailable()).toBe(false);
    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = { invoke: vi.fn() };
    expect(desktopCaptureAvailable()).toBe(true);
  });

  it("asks the app to snapshot the area of its window", async () => {
    document.documentElement.dataset.desktop = "macos";
    const invoke = vi.fn(async () => "data:image/png;base64,AAA=");
    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = { invoke };
    await expect(desktopCapture(rect)).resolves.toBe("data:image/png;base64,AAA=");
    expect(invoke).toHaveBeenCalledWith("capture_region", { region: rect });
    invoke.mockResolvedValueOnce("not an image");
    await expect(desktopCapture(rect)).rejects.toThrow("returned no image");
  });
});
