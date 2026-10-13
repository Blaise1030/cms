/** An element picked in the preview, described for the agent. */
export interface PickedElement {
  /** Short display label: `h1.title`, `section#pricing`. */
  label: string;
  /** The element's opening tag, trimmed: `<h1 class="title">`. */
  tag: string;
  /** A CSS path from the nearest id (or `body`) to the element. */
  selector: string;
  /** Visible text, collapsed and truncated; empty for elements without text. */
  text: string;
  /** Source file of the nearest Astro-annotated ancestor (dev mode only), relative when possible. */
  file?: string;
  /** `line:col` in `file`. */
  loc?: string;
  /** Absolute path of a screenshot of the element, saved by the daemon for the agent. */
  screenshot?: string;
}

const MAX_TEXT = 80;
const MAX_TAG = 120;
const MAX_CLASSES = 2;

/** Classes worth showing: Astro's scoped `astro-xxxx` hashes say nothing to a reader. */
function classes(el: Element): string[] {
  return [...el.classList].filter((c) => !/^astro-[a-z0-9]+$/i.test(c));
}

export function elementLabel(el: Element): string {
  const tag = el.tagName.toLowerCase();
  if (el.id) return `${tag}#${el.id}`;
  const cls = classes(el).slice(0, MAX_CLASSES);
  return cls.length ? `${tag}.${cls.join(".")}` : tag;
}

function openingTag(el: Element): string {
  const attrs = [...el.attributes]
    .filter((a) => !a.name.startsWith("data-astro-"))
    .map((a) => {
      const value = a.name === "class" ? classes(el).join(" ") : a.value;
      return value ? `${a.name}="${value}"` : a.name;
    })
    .filter((a) => a !== 'class=""' && a !== "class");
  const tag = `<${[el.tagName.toLowerCase(), ...attrs].join(" ")}>`;
  return tag.length > MAX_TAG ? `${tag.slice(0, MAX_TAG - 2)}…>` : tag;
}

function cssPath(el: Element): string {
  const parts: string[] = [];
  let node: Element | null = el;
  while (node && node.tagName !== "BODY" && node.tagName !== "HTML") {
    if (node.id) {
      parts.unshift(`#${CSS.escape(node.id)}`);
      return parts.join(" > ");
    }
    const tag = node.tagName.toLowerCase();
    const parent: Element | null = node.parentElement;
    const same = parent ? [...parent.children].filter((c) => c.tagName === node!.tagName) : [];
    parts.unshift(same.length > 1 ? `${tag}:nth-of-type(${same.indexOf(node) + 1})` : tag);
    node = parent;
  }
  parts.unshift("body");
  return parts.join(" > ");
}

/** `/Users/me/workbench-projects/site-1/src/pages/index.astro` → `src/pages/index.astro`. */
function relativeFile(file: string, projectId: string): string {
  const marker = `/${projectId}/`;
  const at = file.lastIndexOf(marker);
  return at >= 0 ? file.slice(at + marker.length) : file;
}

export function describeElement(el: Element, projectId: string): PickedElement {
  // `innerText` follows layout, so sibling blocks (logos, list items) don't run together the
  // way `textContent` would ("Northwind" + "Globex" → "NorthwindGlobex").
  const raw = "innerText" in el ? (el as HTMLElement).innerText : null;
  const text = (raw || el.textContent || "").replace(/\s+/g, " ").trim();
  const source = el.closest("[data-astro-source-file]");
  const file = source?.getAttribute("data-astro-source-file");
  return {
    label: elementLabel(el),
    tag: openingTag(el),
    selector: cssPath(el),
    text: text.length > MAX_TEXT ? `${text.slice(0, MAX_TEXT - 1)}…` : text,
    ...(file
      ? {
          file: relativeFile(file, projectId),
          loc: source!.getAttribute("data-astro-source-loc") ?? undefined,
        }
      : {}),
  };
}

/** The one-line reference the agent receives in place of the composer chip. */
export function elementPrompt(pick: PickedElement): string {
  const where = pick.file
    ? `in ${pick.file}${pick.loc ? `:${pick.loc}` : ""}`
    : `at \`${pick.selector}\``;
  const text = pick.text ? ` "${pick.text}"` : "";
  // Outside the brackets and escaped like a dropped file, so agent CLIs attach the image.
  const shot = pick.screenshot ? ` ${pick.screenshot.replace(/ /g, "\\ ")} ` : "";
  return `[element ${pick.tag}${text} ${where}]${shot}`;
}

const OVERLAY_ID = "__workbench-picker";

/**
 * Hover-highlight elements in a same-origin preview window; a click picks one. Page clicks are
 * swallowed while picking so links and buttons don't fire. Returns a function that stops picking.
 */
export function attachPicker(
  win: Window,
  onPick: (el: Element) => void,
  onCancel: () => void,
): () => void {
  const doc = win.document;
  doc.getElementById(OVERLAY_ID)?.remove();

  const overlay = doc.createElement("div");
  overlay.id = OVERLAY_ID;
  overlay.setAttribute(
    "style",
    "position:fixed;z-index:2147483647;pointer-events:none;display:none;box-sizing:border-box;" +
      "border:2px solid #10b981;background:rgba(16,185,129,.12);border-radius:2px;",
  );
  const badge = doc.createElement("div");
  badge.setAttribute(
    "style",
    "position:absolute;left:-2px;bottom:100%;margin-bottom:2px;padding:1px 6px;border-radius:3px;" +
      "background:#10b981;color:#fff;font:500 11px/16px ui-sans-serif,system-ui,sans-serif;" +
      "white-space:nowrap;",
  );
  overlay.append(badge);
  doc.documentElement.append(overlay);

  const prevCursor = doc.documentElement.style.cursor;
  doc.documentElement.style.cursor = "crosshair";

  let hovered: Element | null = null;

  function place() {
    if (!hovered || !hovered.isConnected) {
      overlay.style.display = "none";
      return;
    }
    const r = hovered.getBoundingClientRect();
    Object.assign(overlay.style, {
      display: "block",
      top: `${r.top}px`,
      left: `${r.left}px`,
      width: `${r.width}px`,
      height: `${r.height}px`,
    });
    badge.textContent = elementLabel(hovered);
    // Keep the badge on screen for elements touching the top edge.
    Object.assign(
      badge.style,
      r.top < 20 ? { bottom: "auto", top: "100%", marginTop: "2px" } : { bottom: "100%", top: "auto" },
    );
  }

  function target(e: Event): Element | null {
    // The frame has its own `Element` constructor, so check the node type instead of instanceof.
    const el = e.target as Node | null;
    if (!el || el.nodeType !== 1 || el === doc.documentElement || el === doc.body) return null;
    return el as Element;
  }

  function onMove(e: MouseEvent) {
    hovered = target(e);
    place();
  }

  function swallow(e: Event) {
    e.preventDefault();
    e.stopPropagation();
  }

  function onClick(e: MouseEvent) {
    swallow(e);
    const el = target(e);
    if (el) onPick(el);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    swallow(e);
    onCancel();
  }

  const opts = { capture: true };
  doc.addEventListener("mousemove", onMove, opts);
  doc.addEventListener("click", onClick, opts);
  // Stop the press itself too, so drags, focus and `mousedown` handlers stay quiet.
  doc.addEventListener("mousedown", swallow, opts);
  doc.addEventListener("pointerdown", swallow, opts);
  doc.addEventListener("keydown", onKey, opts);
  win.addEventListener("scroll", place, opts);
  win.addEventListener("resize", place);

  return () => {
    doc.removeEventListener("mousemove", onMove, opts);
    doc.removeEventListener("click", onClick, opts);
    doc.removeEventListener("mousedown", swallow, opts);
    doc.removeEventListener("pointerdown", swallow, opts);
    doc.removeEventListener("keydown", onKey, opts);
    win.removeEventListener("scroll", place, opts);
    win.removeEventListener("resize", place);
    doc.documentElement.style.cursor = prevCursor;
    overlay.remove();
  };
}
