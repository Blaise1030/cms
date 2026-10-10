/** Section files have stable IDs in the Astro template; shared files compare as whole pages. */
export function sectionIdForFile(filePath: string): string | null {
  return /^src\/content\/sections\/([a-z0-9-]+)\.mdx$/.exec(filePath)?.[1] ?? null;
}

export function findConflictTarget(doc: Document, filePath: string, sectionId?: string): Element | null {
  const id = sectionId ?? sectionIdForFile(filePath);
  if (!id || !/^[a-z0-9-]+$/.test(id)) return null;
  const matches = doc.querySelectorAll(`section[id="${id}"]`);
  return matches.length === 1 ? matches[0]! : null;
}

export function attachConflictHighlight(win: Window, target: Element | null): () => void {
  if (!target) return () => {};
  const overlay = win.document.createElement("div");
  overlay.setAttribute("data-conflict-highlight", "");
  Object.assign(overlay.style, {
    position: "fixed", pointerEvents: "none", zIndex: "2147483647",
    border: "3px solid #f59e0b", borderRadius: "8px", boxShadow: "0 0 0 4px #f59e0b44",
  });
  win.document.documentElement.append(overlay);
  const position = () => {
    const rect = target.getBoundingClientRect();
    Object.assign(overlay.style, { top: `${rect.top}px`, left: `${rect.left}px`, width: `${rect.width}px`, height: `${rect.height}px` });
  };
  position();
  win.addEventListener("scroll", position, true);
  win.addEventListener("resize", position);
  return () => { win.removeEventListener("scroll", position, true); win.removeEventListener("resize", position); overlay.remove(); };
}
