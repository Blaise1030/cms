export interface FrameAppearance {
  rtl: boolean;
  dark: boolean;
}

/** The page's own `<html>` state, restored when an override is turned off. */
interface Original {
  dir: string | null;
  dark: boolean;
}

const originals = new WeakMap<Document, Original>();

/**
 * Override `dir` and the `dark` class (Tailwind's class strategy) on the frame's `<html>`.
 * `prefers-color-scheme` is driven separately, by the iframe element's `color-scheme`.
 * Pass `fresh` after the page replaced `<html>` attributes itself (Astro's client router).
 */
export function applyFrameAppearance(doc: Document, { rtl, dark }: FrameAppearance, fresh = false) {
  const root = doc.documentElement;
  let original = fresh ? undefined : originals.get(doc);
  if (!original) {
    original = { dir: root.getAttribute("dir"), dark: root.classList.contains("dark") };
    originals.set(doc, original);
  }
  const dir = rtl ? "rtl" : original.dir;
  if (dir === null) root.removeAttribute("dir");
  else root.setAttribute("dir", dir);
  root.classList.toggle("dark", dark || original.dark);
}
