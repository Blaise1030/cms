/**
 * macOS keeps scrollbars on screen when the system is set to always show them. The app reveals
 * its own scrollbar only while the scroller is moving: one capture listener covers every scroller,
 * because `scroll` does not bubble but does capture.
 */
export const SCROLLBAR_VISIBLE_MS = 700;

export function installScrollbarHiding(doc: Document = document): () => void {
  let hide: ReturnType<typeof setTimeout> | undefined;
  let shown: Element | null = null;

  const onScroll = (event: Event) => {
    const target = event.target;
    const element = target instanceof Element ? target : doc.documentElement;
    if (shown && shown !== element) shown.removeAttribute("data-scrolling");
    shown = element;
    element.setAttribute("data-scrolling", "");
    clearTimeout(hide);
    hide = setTimeout(() => {
      element.removeAttribute("data-scrolling");
      if (shown === element) shown = null;
    }, SCROLLBAR_VISIBLE_MS);
  };

  doc.addEventListener("scroll", onScroll, { capture: true, passive: true });
  return () => {
    clearTimeout(hide);
    shown?.removeAttribute("data-scrolling");
    doc.removeEventListener("scroll", onScroll, { capture: true });
  };
}
