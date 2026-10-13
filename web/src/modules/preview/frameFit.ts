import { DEVICE_WIDTH, type Device } from "./store";

/** Desktop renders at least this wide; a narrower panel shows it scaled down instead of reflowed. */
export const DESKTOP_MIN_WIDTH = 1440;

export interface FrameFit {
  /** Layout width of the framed page in CSS px; `null` fills the panel. */
  width: number | null;
  /** Layout height in CSS px; `null` fills the panel. */
  height: number | null;
  /** Visual scale applied to the frame (≤ 1). */
  scale: number;
}

/**
 * Size the preview frame for `device` in a panel of `panelWidth` × `panelHeight`: the page is
 * laid out at the device width and, when the panel is narrower, scaled down to fit it, so a
 * narrow panel still shows the real desktop (or tablet) layout. Never scales up. An unmeasured
 * panel (0) keeps the plain device width.
 */
export function frameFit(device: Device, panelWidth: number, panelHeight: number): FrameFit {
  const fixed = DEVICE_WIDTH[device];
  const target = fixed ?? DESKTOP_MIN_WIDTH;
  if (panelWidth <= 0 || panelHeight <= 0 || panelWidth >= target) {
    return { width: fixed, height: null, scale: 1 };
  }
  const scale = panelWidth / target;
  return { width: target, height: panelHeight / scale, scale };
}
