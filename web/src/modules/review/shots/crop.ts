/** A rectangle in CSS pixels of the page (`clientX`/`clientY` space). */
export interface CssRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A rectangle in the captured video's pixels. */
export interface PixelRect {
  sx: number;
  sy: number;
  sw: number;
  sh: number;
}

/** A drag shorter than this on either side is a click, not a selection. */
export const MIN_SELECTION = 8;
/** Longest side of a screenshot sent to GitHub. */
export const MAX_SIDE = 1600;
/** Above this a PNG is re-encoded as JPEG. */
export const MAX_PNG_BYTES = 1024 * 1024;
export const JPEG_QUALITY = 0.85;

/** The rectangle between two pointer positions, whichever way the drag went. */
export function dragRect(x0: number, y0: number, x1: number, y1: number): CssRect {
  return { x: Math.min(x0, x1), y: Math.min(y0, y1), width: Math.abs(x1 - x0), height: Math.abs(y1 - y0) };
}

export const isSelection = (r: CssRect) => r.width >= MIN_SELECTION && r.height >= MIN_SELECTION;

/**
 * Where a CSS-pixel rectangle of the page lies in a capture of the tab. The capture is the tab's
 * viewport at its own resolution (devicePixelRatio times the CSS size, or whatever the browser
 * chose), so the scale is the video's size over the viewport's, per axis. Clamped to the frame.
 */
export function cropRect(
  rect: CssRect,
  video: { width: number; height: number },
  viewport: { width: number; height: number },
): PixelRect {
  const kx = video.width / Math.max(viewport.width, 1);
  const ky = video.height / Math.max(viewport.height, 1);
  const sx = Math.max(0, Math.round(rect.x * kx));
  const sy = Math.max(0, Math.round(rect.y * ky));
  const sw = Math.max(1, Math.min(video.width - sx, Math.round(rect.width * kx)));
  const sh = Math.max(1, Math.min(video.height - sy, Math.round(rect.height * ky)));
  return { sx, sy, sw, sh };
}

/** The output size for a crop: unchanged when it fits, else shrunk so its longest side is 1600. */
export function outputSize(sw: number, sh: number, max = MAX_SIDE): { width: number; height: number } {
  const k = Math.min(1, max / Math.max(sw, sh, 1));
  return { width: Math.max(1, Math.round(sw * k)), height: Math.max(1, Math.round(sh * k)) };
}
