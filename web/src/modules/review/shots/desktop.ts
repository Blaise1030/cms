import { isMacDesktop } from "@/lib/desktop";
import type { CssRect } from "./crop";

type TauriInternals = { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
const tauri = () => (window as unknown as { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__;

/** In the macOS desktop app, which can snapshot its own window without asking. */
export function desktopCaptureAvailable(): boolean {
  return isMacDesktop() && typeof tauri()?.invoke === "function";
}

/** A PNG data URL of `rect` (CSS px of the window), from the desktop app's native snapshot. */
export async function desktopCapture(rect: CssRect): Promise<string> {
  const internals = tauri();
  if (!isMacDesktop() || !internals) throw new Error("Not in the desktop app.");
  const url = await internals.invoke("capture_region", { region: rect });
  if (typeof url !== "string" || !url.startsWith("data:image/")) throw new Error("The desktop app returned no image.");
  return url;
}
