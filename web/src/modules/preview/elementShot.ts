import { domToBlob } from "modern-screenshot";

/** Longest side of a screenshot, in pixels: enough to read, small enough for the agent. */
const MAX_SIDE = 2000;

/** The pixel scale for an element: retina-sharp when it fits, shrunk when it would not. */
export function shotScale(width: number, height: number, dpr: number): number {
  const side = Math.max(width, height, 1);
  return Math.min(Math.max(dpr, 1), MAX_SIDE / side);
}

/** A PNG of `el` as rendered in its (same-origin) frame; null for elements with no area. */
export async function captureElement(el: Element): Promise<Blob | null> {
  const { width, height } = el.getBoundingClientRect();
  if (width < 1 || height < 1) return null;
  const win = el.ownerDocument.defaultView;
  return domToBlob(el, {
    scale: shotScale(width, height, win?.devicePixelRatio ?? 1),
    type: "image/png",
  });
}
