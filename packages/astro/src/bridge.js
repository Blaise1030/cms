/**
 * Preview bridge, injected inline into every page by `orso()` during `astro dev`.
 *
 * The dev UI frames the project's dev server directly (so Astro islands and other client
 * modules hydrate), which makes the frame cross-origin for the app. This script lets the
 * app drive the element picker and the appearance overrides without framing the proxied
 * preview. It does nothing unless the parent posts an `orso:preview` message.
 */
(() => {
  const TYPE = "orso:preview";
  const OVERLAY_ID = "__orso-picker";
  const MAX_TEXT = 80;
  const MAX_TAG = 120;
  const MAX_CLASSES = 2;

  let parentOrigin = null;
  let picking = false;
  let overlay = null;
  let badge = null;
  let hovered = null;
  let prevCursor = "";
  let appearance = null;
  let original = null;

  const isLoopback = (origin) => {
    try {
      const host = new URL(origin).hostname;
      return (
        host === "localhost" ||
        host === "127.0.0.1" ||
        host === "::1" ||
        host === "[::1]" ||
        host.endsWith(".localhost")
      );
    } catch {
      return false;
    }
  };

  const classes = (el) => [...el.classList].filter((c) => !/^astro-[a-z0-9]+$/i.test(c));

  function label(el) {
    const tag = el.tagName.toLowerCase();
    if (el.id) return `${tag}#${el.id}`;
    const cls = classes(el).slice(0, MAX_CLASSES);
    return cls.length ? `${tag}.${cls.join(".")}` : tag;
  }

  function openingTag(el) {
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

  function selector(el) {
    const parts = [];
    let node = el;
    while (node && node.tagName !== "BODY" && node.tagName !== "HTML") {
      if (node.id) {
        parts.unshift(`#${CSS.escape(node.id)}`);
        return parts.join(" > ");
      }
      const tag = node.tagName.toLowerCase();
      const parent = node.parentElement;
      const same = parent ? [...parent.children].filter((c) => c.tagName === node.tagName) : [];
      parts.unshift(same.length > 1 ? `${tag}:nth-of-type(${same.indexOf(node) + 1})` : tag);
      node = parent;
    }
    parts.unshift("body");
    return parts.join(" > ");
  }

  function describe(el) {
    const raw = "innerText" in el ? el.innerText : null;
    const text = (raw || el.textContent || "").replace(/\s+/g, " ").trim();
    const source = el.closest("[data-astro-source-file]");
    const file = source && source.getAttribute("data-astro-source-file");
    const loc = source && source.getAttribute("data-astro-source-loc");
    const element = {
      label: label(el),
      tag: openingTag(el),
      selector: selector(el),
      text: text.length > MAX_TEXT ? `${text.slice(0, MAX_TEXT - 1)}…` : text,
    };
    if (file) element.file = file;
    if (loc) element.loc = loc;
    return element;
  }

  function place() {
    if (!hovered || !hovered.isConnected) {
      if (overlay) overlay.style.display = "none";
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
    badge.textContent = label(hovered);
    Object.assign(
      badge.style,
      r.top < 20 ? { bottom: "auto", top: "100%", marginTop: "2px" } : { bottom: "100%", top: "auto" },
    );
  }

  function target(e) {
    const el = e.target;
    if (!el || el.nodeType !== 1 || el === document.documentElement || el === document.body) return null;
    return el;
  }

  function swallow(e) {
    e.preventDefault();
    e.stopPropagation();
  }

  function onMove(e) {
    hovered = target(e);
    place();
  }

  function onClick(e) {
    swallow(e);
    const el = target(e);
    if (!el) return;
    const element = describe(el);
    stop();
    post("pick", { element });
  }

  function onKey(e) {
    if (e.key !== "Escape") return;
    swallow(e);
    stop();
    post("cancel");
  }

  const capture = { capture: true };

  function start() {
    if (picking) return;
    picking = true;
    document.getElementById(OVERLAY_ID)?.remove();
    overlay = document.createElement("div");
    overlay.id = OVERLAY_ID;
    overlay.setAttribute(
      "style",
      "position:fixed;z-index:2147483647;pointer-events:none;display:none;box-sizing:border-box;" +
        "border:2px solid #10b981;background:rgba(16,185,129,.12);border-radius:2px;",
    );
    badge = document.createElement("div");
    badge.setAttribute(
      "style",
      "position:absolute;left:-2px;bottom:100%;margin-bottom:2px;padding:1px 6px;border-radius:3px;" +
        "background:#10b981;color:#fff;font:500 11px/16px ui-sans-serif,system-ui,sans-serif;" +
        "white-space:nowrap;",
    );
    overlay.append(badge);
    document.documentElement.append(overlay);
    prevCursor = document.documentElement.style.cursor;
    document.documentElement.style.cursor = "crosshair";
    document.addEventListener("mousemove", onMove, capture);
    document.addEventListener("click", onClick, capture);
    document.addEventListener("mousedown", swallow, capture);
    document.addEventListener("pointerdown", swallow, capture);
    document.addEventListener("keydown", onKey, capture);
    addEventListener("scroll", place, capture);
    addEventListener("resize", place);
  }

  function stop() {
    if (!picking) return;
    picking = false;
    hovered = null;
    document.removeEventListener("mousemove", onMove, capture);
    document.removeEventListener("click", onClick, capture);
    document.removeEventListener("mousedown", swallow, capture);
    document.removeEventListener("pointerdown", swallow, capture);
    document.removeEventListener("keydown", onKey, capture);
    removeEventListener("scroll", place, capture);
    removeEventListener("resize", place);
    document.documentElement.style.cursor = prevCursor;
    if (overlay) overlay.remove();
    overlay = null;
    badge = null;
  }

  function applyAppearance(next) {
    appearance = next;
    const root = document.documentElement;
    if (!original) {
      original = { dir: root.getAttribute("dir"), dark: root.classList.contains("dark") };
    }
    const dir = next.rtl ? "rtl" : original.dir;
    if (dir === null) root.removeAttribute("dir");
    else root.setAttribute("dir", dir);
    root.classList.toggle("dark", !!next.dark || original.dark);
  }

  function post(op, extra) {
    parent.postMessage(Object.assign({ type: TYPE, op }, extra), parentOrigin || "*");
  }

  addEventListener("message", (e) => {
    if (e.source !== parent || !isLoopback(e.origin) || !e.data || e.data.type !== TYPE) return;
    parentOrigin = e.origin;
    if (e.data.op === "pick:start") start();
    else if (e.data.op === "pick:stop") stop();
    else if (e.data.op === "appearance") {
      applyAppearance({ rtl: !!e.data.rtl, dark: !!e.data.dark });
    }
  });

  // Astro's router copies the next page's `<html>` attributes over ours.
  for (const event of ["astro:page-load", "astro:after-swap"]) {
    document.addEventListener(event, () => {
      if (appearance) applyAppearance(appearance);
    });
  }
})();
