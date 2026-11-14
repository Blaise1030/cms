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
  if (document.querySelector(`[data-testid="${ROOT_TEST_ID}"]`)) return;

  const host = document.createElement("div");
  host.dataset.testid = ROOT_TEST_ID;
  host.setAttribute("data-orso-checks", "panel");
  Object.assign(host.style, {
    position: "fixed",
    right: "12px",
    bottom: "12px",
    zIndex: "2147483646",
    fontFamily: 'ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif',
    fontSize: "12px",
    lineHeight: "1.4",
    color: "#0f172a",
  });

  const shadow = host.attachShadow({ mode: "open" });

  const style = document.createElement("style");
  style.textContent = `
    :host { all: initial; }
    * { box-sizing: border-box; }
    button {
      font: inherit;
      cursor: pointer;
      border: 1px solid #cbd5e1;
      border-radius: 6px;
      background: #fff;
      color: #0f172a;
      padding: 6px 10px;
    }
    button:hover { background: #f8fafc; }
  .trigger {
      border-radius: 999px;
      box-shadow: 0 4px 14px rgb(15 23 42 / 0.12);
      font-weight: 600;
    }
    .drawer {
      display: none;
      flex-direction: column;
      gap: 8px;
      width: min(320px, calc(100vw - 24px));
      max-height: min(420px, calc(100vh - 80px));
      margin-bottom: 8px;
      padding: 10px;
      border: 1px solid #e2e8f0;
      border-radius: 8px;
      background: #fff;
      box-shadow: 0 8px 24px rgb(15 23 42 / 0.15);
      overflow: hidden;
    }
    .drawer.open { display: flex; }
    .head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 8px;
      font-weight: 600;
    }
    .summary { color: #64748b; font-size: 11px; }
    .findings {
      overflow: auto;
      display: flex;
      flex-direction: column;
      gap: 6px;
      margin: 0;
      padding: 0;
      list-style: none;
    }
    .finding {
      padding: 6px 8px;
      border-radius: 6px;
      border: 1px solid #e2e8f0;
      background: #f8fafc;
    }
    .sev-error { border-left: 3px solid #dc2626; }
    .sev-warning { border-left: 3px solid #d97706; }
    .sev {
      display: inline-block;
      font-size: 10px;
      font-weight: 700;
      text-transform: uppercase;
      margin-right: 6px;
    }
    .empty { color: #64748b; font-style: italic; }
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
      item.append(sev, document.createTextNode(f.message));
      if (f.detail) {
        item.append(document.createTextNode(` ${f.detail}`));
      }
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

  const observer = new MutationObserver(applyPickState);
  observer.observe(document.documentElement, { childList: true, subtree: true });
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