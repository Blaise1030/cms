import { sortFindings } from "../checks/findings.js";
import { auditPage } from "../checks/page.js";

const PICKER_ID = "__orso-picker";
const ROOT_TEST_ID = "orso-checks-panel";

function pagePath() {
  const path = `${location.pathname}${location.search}`;
  return path === "" ? "/" : path;
}

/**
 * @param {Document} doc
 */
function resourceSummary(doc) {
  return {
    scripts: doc.querySelectorAll("script").length,
    styles: doc.querySelectorAll('link[rel="stylesheet"], style').length,
    images: doc.querySelectorAll("img").length,
    htmlBytes: new TextEncoder().encode(doc.documentElement.outerHTML).length,
  };
}

function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function mount() {
  // Replace any prior host so HMR / full reload always picks up new styles.
  document.querySelectorAll(`[data-testid="${ROOT_TEST_ID}"]`).forEach((el) => el.remove());

  const host = document.createElement("div");
  host.dataset.testid = ROOT_TEST_ID;
  host.setAttribute("data-orso-checks", "panel");
  Object.assign(host.style, {
    position: "fixed",
    right: "12px",
    bottom: "12px",
    zIndex: "2147483646",
  });

  const shadow = host.attachShadow({ mode: "open" });

  const style = document.createElement("style");
  // Default to orso app dark chrome (`web/src/index.css` `.dark`). Light only when the
  // preview is not in dark appearance.
  style.textContent = `
    :host { all: initial; }
    * { box-sizing: border-box; }
    .wrap {
      --background: oklch(0.18 0 0);
      --foreground: oklch(0.985 0 0);
      --card: oklch(0.205 0 0);
      --card-foreground: oklch(0.985 0 0);
      --muted: oklch(0.269 0 0);
      --muted-foreground: oklch(0.708 0 0);
      --border: oklch(1 0 0 / 10%);
      --primary: oklch(0.62 0.19 260);
      --primary-foreground: oklch(0.985 0 0);
      --destructive: oklch(0.704 0.191 22.216);
      --warning: oklch(0.8 0.14 75);
      --radius: 0.625rem;
      --shadow: 0 12px 32px oklch(0 0 0 / 0.45);
      color-scheme: dark;
      font-family: "Geist Variable", ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji";
      font-size: 0.8125rem;
      line-height: 1.35;
      color: var(--foreground);
    }
    .wrap.light {
      --background: oklch(0.98 0 0);
      --foreground: oklch(0.145 0 0);
      --card: oklch(1 0 0);
      --card-foreground: oklch(0.145 0 0);
      --muted: oklch(0.97 0 0);
      --muted-foreground: oklch(0.556 0 0);
      --border: oklch(0.922 0 0);
      --primary: oklch(0.546 0.215 263);
      --primary-foreground: oklch(0.985 0 0);
      --destructive: oklch(0.577 0.245 27.325);
      --warning: oklch(0.75 0.15 75);
      --shadow: 0 10px 30px oklch(0.145 0 0 / 0.12);
      color-scheme: light;
    }
    button {
      font: inherit;
      cursor: pointer;
      border: 1px solid var(--border);
      border-radius: calc(var(--radius) - 2px);
      background: var(--muted);
      color: var(--foreground);
      padding: 0.35rem 0.7rem;
    }
    button:hover { filter: brightness(1.06); }
    button:focus-visible {
      outline: 2px solid color-mix(in oklch, var(--primary) 55%, transparent);
      outline-offset: 2px;
    }
    .trigger {
      border-radius: 999px;
      background: var(--card);
      border-color: var(--border);
      box-shadow: var(--shadow);
      font-weight: 550;
      padding: 0.45rem 0.9rem;
    }
    .drawer {
      display: none;
      flex-direction: column;
      gap: 0.65rem;
      width: min(22rem, calc(100vw - 24px));
      max-height: min(26rem, calc(100vh - 80px));
      margin-bottom: 0.5rem;
      padding: 0.75rem;
      border: 1px solid var(--border);
      border-radius: var(--radius);
      background: var(--card);
      color: var(--card-foreground);
      box-shadow: var(--shadow);
      overflow: hidden;
    }
    .drawer.open { display: flex; }
    .head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 0.5rem;
      font-weight: 550;
      letter-spacing: -0.01em;
    }
    .head button {
      background: transparent;
      color: var(--muted-foreground);
      border-color: var(--border);
      font-size: 0.75rem;
      font-weight: 500;
      padding: 0.25rem 0.55rem;
    }
    .head button:hover {
      color: var(--foreground);
      background: var(--muted);
    }
    .summary { color: var(--muted-foreground); font-size: 0.72rem; }
    .findings {
      overflow: auto;
      display: flex;
      flex-direction: column;
      gap: 0.4rem;
      margin: 0;
      padding: 0;
      list-style: none;
    }
    .finding {
      display: flex;
      align-items: flex-start;
      gap: 0.5rem;
      padding: 0.55rem 0.65rem;
      border-radius: calc(var(--radius) - 2px);
      border: 1px solid var(--border);
      background: color-mix(in oklch, var(--muted) 65%, var(--card));
    }
    .sev {
      display: inline-flex;
      align-items: center;
      flex-shrink: 0;
      margin-top: 0.05rem;
      padding: 0.1rem 0.45rem;
      border-radius: 999px;
      border: 1px solid transparent;
      font-size: 0.65rem;
      font-weight: 550;
      letter-spacing: 0.02em;
      text-transform: uppercase;
    }
    .sev-error .sev {
      color: oklch(0.985 0 0);
      background: color-mix(in oklch, var(--destructive) 85%, black);
    }
    .sev-warning .sev {
      color: oklch(0.22 0.05 75);
      background: color-mix(in oklch, var(--warning) 72%, white);
    }
    .msg { min-width: 0; color: var(--card-foreground); }
    .detail {
      display: block;
      margin-top: 0.2rem;
      color: var(--muted-foreground);
      font-size: 0.72rem;
    }
    .empty { color: var(--muted-foreground); padding: 0.35rem 0.15rem; }
    .picking { opacity: 0.35; pointer-events: none; }
  `;

  const wrap = document.createElement("div");
  wrap.className = "wrap";

  const drawer = document.createElement("div");
  drawer.className = "drawer";
  drawer.setAttribute("data-orso-checks-drawer", "");

  const head = document.createElement("div");
  head.className = "head";
  head.textContent = "Preview checks";

  const refresh = document.createElement("button");
  refresh.type = "button";
  refresh.textContent = "Refresh";
  refresh.setAttribute("data-orso-checks-refresh", "");
  head.append(refresh);

  const summary = document.createElement("div");
  summary.className = "summary";
  summary.setAttribute("data-orso-checks-summary", "");

  const list = document.createElement("ul");
  list.className = "findings";
  list.setAttribute("data-orso-checks-findings", "");

  drawer.append(head, summary, list);

  const trigger = document.createElement("button");
  trigger.type = "button";
  trigger.className = "trigger";
  trigger.textContent = "Checks";
  trigger.setAttribute("data-orso-checks-trigger", "");

  wrap.append(drawer, trigger);
  shadow.append(style, wrap);
  document.documentElement.append(host);

  let open = false;
  let ranOnce = false;

  function syncAppearance() {
    // Orso chrome defaults to dark; only go light when the preview is not dark.
    wrap.classList.toggle("light", !document.documentElement.classList.contains("dark"));
  }
  syncAppearance();

  /** @param {import("../checks/findings.js").Finding[]} findings */
  function render(findings) {
    const { scripts, styles, images, htmlBytes } = resourceSummary(document);
    summary.textContent = `${scripts} scripts · ${styles} styles · ${images} images · HTML ${formatBytes(htmlBytes)}`;

    list.replaceChildren();
    const sorted = sortFindings(findings);
    if (sorted.length === 0) {
      const item = document.createElement("li");
      item.className = "empty";
      item.textContent = "No issues found on this page.";
      list.append(item);
      return;
    }
    for (const f of sorted) {
      const item = document.createElement("li");
      item.className = `finding sev-${f.severity}`;
      const sev = document.createElement("span");
      sev.className = "sev";
      sev.textContent = f.severity;
      const msg = document.createElement("div");
      msg.className = "msg";
      msg.append(document.createTextNode(f.message));
      if (f.detail) {
        const detail = document.createElement("span");
        detail.className = "detail";
        detail.textContent = f.detail;
        msg.append(detail);
      }
      item.append(sev, msg);
      list.append(item);
    }
  }

  function runAudit() {
    const findings = auditPage(document, { path: pagePath() });
    render(findings);
    ranOnce = true;
  }

  function setOpen(next) {
    open = next;
    drawer.classList.toggle("open", open);
    trigger.setAttribute("aria-expanded", open ? "true" : "false");
    if (open && !ranOnce) runAudit();
  }

  trigger.addEventListener("click", () => setOpen(!open));
  refresh.addEventListener("click", () => {
    if (open) runAudit();
  });

  function applyPickState() {
    const picking = document.getElementById(PICKER_ID) !== null;
    wrap.classList.toggle("picking", picking);
    if (picking && open) setOpen(false);
  }

  const observer = new MutationObserver(() => {
    applyPickState();
    syncAppearance();
  });
  observer.observe(document.documentElement, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["class"],
  });
  applyPickState();

  setOpen(false);
}

if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", mount, { once: true });
  } else {
    mount();
  }
}