/**
 * Preview bridge, injected inline into every page by `orso()` during `astro dev` and into
 * pull request preview builds (`ORSO_PREVIEW=1`), together with `capture.js`.
 *
 * orso serves every preview at the root of an origin of its own, so the app's frame is always
 * cross-origin and the app cannot touch the page. This script is the only way in: it drives
 * the element picker and the appearance overrides, reports the page's address, its errors and
 * its HTML (Page details), and screenshots areas of itself (picked elements, review shots)
 * without asking to share the screen.
 *
 * It talks only to orso. Under orso's dev server, `__orsoAppOrigins` names the app origins
 * (`ORSO_APP_ORIGINS`, set by the daemon): messages from any other origin are ignored, and
 * messages go out to those origins by name, never to `*`. A deployed preview build has no
 * such list and answers a loopback parent (orso's own app) once it has said hello.
 */
(() => {
  const TYPE = "orso:preview";
  /** Exact app origins, when the daemon named them. */
  const APP_ORIGINS =
    typeof __orsoAppOrigins !== "undefined" && Array.isArray(__orsoAppOrigins) ? __orsoAppOrigins : [];
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
  const OVERLAY_TAG = "vite-error-overlay";
  const MAX_MESSAGE = 1000;
  const MAX_STACK_LINES = 12;

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

  /** Whether a message from `origin` is from orso's app. */
  const trusted = (origin) => (APP_ORIGINS.length ? APP_ORIGINS.includes(origin) : isLoopback(origin));

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

  async function onClick(e) {
    swallow(e);
    const el = target(e);
    if (!el) return;
    const element = describe(el);
    stop();
    // A picture of the element goes with the mention when the page can take one.
    const shot = await shoot(el);
    if (shot) element.screenshot = shot;
    post("pick", { element });
  }

  /** A PNG data URL of the visible part of `el`, or null. */
  async function shoot(el) {
    if (typeof __orsoCaptureRect !== "function") return null;
    const r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) return null;
    try {
      return await __orsoCaptureRect({ x: r.left, y: r.top, width: r.width, height: r.height });
    } catch {
      // The mention is still useful without the picture.
      return null;
    }
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

  /** The framing page's origin as the referrer names it, when it is one of the app origins. */
  const referrerOrigin = (() => {
    try {
      const origin = new URL(document.referrer).origin;
      return APP_ORIGINS.includes(origin) ? origin : null;
    } catch {
      return null;
    }
  })();

  /**
   * Send to the app: to the origin that last spoke to us, else to the app origin the referrer
   * names, else to each named app origin (the browser delivers only to the one that matches the
   * parent). Never to `*`: a page that frames this one without being orso hears nothing.
   */
  function post(op, extra) {
    const message = Object.assign({ type: TYPE, op }, extra);
    const targets = parentOrigin ? [parentOrigin] : referrerOrigin ? [referrerOrigin] : APP_ORIGINS;
    for (const origin of targets) {
      try {
        parent.postMessage(message, origin);
      } catch {
        // A malformed origin in the list: skip it.
      }
    }
  }

  /** An area of this page (its own CSS px) as a PNG, or why not. */
  async function captureArea(id, rect) {
    if (typeof __orsoCaptureRect !== "function") {
      post("capture", { id, ok: false, error: "This preview can't capture itself." });
      return;
    }
    try {
      const r = rect || {};
      const area = { x: +r.x || 0, y: +r.y || 0, width: +r.width || 0, height: +r.height || 0 };
      post("capture", { id, ok: true, dataUrl: await __orsoCaptureRect(area) });
    } catch (err) {
      post("capture", { id, ok: false, error: String((err && err.message) || err) });
    }
  }

  function postSnapshot() {
    post("snapshot", {
      url: location.href,
      html: document.documentElement.outerHTML,
    });
  }

  addEventListener("message", (e) => {
    if (e.source !== parent || !trusted(e.origin) || !e.data || e.data.type !== TYPE) return;
    parentOrigin = e.origin;
    if (e.data.op === "pick:start") start();
    else if (e.data.op === "pick:stop") stop();
    else if (e.data.op === "appearance") {
      applyAppearance({ rtl: !!e.data.rtl, dark: !!e.data.dark });
    } else if (e.data.op === "snapshot") {
      postSnapshot();
    } else if (e.data.op === "hello") {
      // The handshake: orso learns the bridge is here and whether it can capture itself, then
      // where the page is and what went wrong so far (earlier reports may have had no origin
      // to go to).
      post("ready", { version: 2, capture: typeof __orsoCaptureRect === "function" });
      reportPath();
      if (lastError) post("error", { error: lastError });
    } else if (e.data.op === "capture") {
      captureArea(e.data.id, e.data.rect);
    }
  });

  // Astro's router copies the next page's `<html>` attributes over ours.
  for (const event of ["astro:page-load", "astro:after-swap"]) {
    document.addEventListener(event, () => {
      if (appearance) applyAppearance(appearance);
    });
  }

  // A cross-origin frame hides its location from the app, so the app cannot keep the reader's
  // page across a reload unless the page says where it is. Report on every navigation.
  function reportPath() {
    post("path", { url: location.href });
  }
  addEventListener("load", reportPath);
  addEventListener("popstate", reportPath);
  addEventListener("hashchange", reportPath);
  document.addEventListener("astro:page-load", reportPath);
  reportPath();

  // --- Page errors: the dev server's error overlay and uncaught errors -----------------------

  let lastError = null;
  let overlayEl = null;

  function clip(text, max = MAX_MESSAGE) {
    const t = text == null ? "" : String(text).trim();
    if (!t) return undefined;
    return t.length > max ? `${t.slice(0, max)}…` : t;
  }

  function clipStack(stack) {
    const lines = stack == null ? [] : String(stack).trim().split("\n");
    if (!lines.length || !lines[0]) return undefined;
    return lines.length > MAX_STACK_LINES ? [...lines.slice(0, MAX_STACK_LINES), "    …"].join("\n") : lines.join("\n");
  }

  function firstText(root, selectors) {
    for (const s of selectors) {
      const el = root.querySelector(s);
      const text = clip(el && el.textContent);
      if (text) return text;
    }
    return undefined;
  }

  /** Read an error overlay: Astro's ids first, then Vite's classes. */
  function readOverlay(el) {
    const root = el.shadowRoot || el;
    const title = firstText(root, ["#title"]);
    const body = firstText(root, ["#message-content", ".message-body"]);
    const message =
      [title !== "An error occurred." ? title : undefined, body].filter(Boolean).join(": ") ||
      clip(root.textContent) ||
      "The page failed to build.";
    // Astro puts the absolute location in the code header's `title`.
    const header = root.querySelector("#code header h2");
    const file = clip(header && header.getAttribute("title")) || firstText(root, ["#code header h2", ".file"]);
    const error = { kind: "build", message };
    if (file) error.file = file;
    const stack = clipStack(firstText(root, ["#stack-content", ".stack"]));
    if (stack) error.stack = stack;
    const hint = firstText(root, ["#hint-content"]);
    if (hint) error.hint = hint;
    return error;
  }

  function fromThrown(reason, fallback) {
    if (reason && typeof reason === "object" && "message" in reason) {
      const error = { kind: "runtime", message: clip(`${reason.name || "Error"}: ${reason.message}`) };
      const stack = clipStack(reason.stack);
      if (stack) error.stack = stack;
      return error;
    }
    return { kind: "runtime", message: clip(reason) || fallback };
  }

  function reportError(error) {
    lastError = error;
    post("error", { error });
  }

  function scanOverlay() {
    const found = document.querySelector(OVERLAY_TAG);
    if (found === overlayEl) return;
    overlayEl = found;
    if (found) reportError(readOverlay(found));
    else if (lastError && lastError.kind === "build") {
      lastError = null;
      post("error-cleared");
    }
  }

  addEventListener("error", (e) => {
    // Resource load errors (a missing image) are not script errors.
    if (typeof ErrorEvent === "function" && !(e instanceof ErrorEvent)) return;
    const error = e.error ? fromThrown(e.error, e.message) : fromThrown(e.message, "Script error");
    if (e.filename) error.file = [e.filename, e.lineno, e.colno].filter(Boolean).join(":");
    reportError(error);
  });
  addEventListener("unhandledrejection", (e) => reportError(fromThrown(e.reason, "Unhandled promise rejection")));
  if (typeof MutationObserver === "function" && document.documentElement) {
    new MutationObserver(scanOverlay).observe(document.documentElement, { childList: true, subtree: true });
    scanOverlay();
  }
})();
