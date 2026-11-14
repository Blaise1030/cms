import { createApp } from "vue";
import ChecksPanel from "./ChecksPanel.vue";
import "./styles.css";

declare const __ORSO_CHECKS_CSS__: string;

const ROOT_TEST_ID = "orso-checks-panel-host";

function mount() {
  document.querySelectorAll(`[data-testid="${ROOT_TEST_ID}"]`).forEach((el) => el.remove());

  const host = document.createElement("div");
  host.dataset.testid = ROOT_TEST_ID;
  host.setAttribute("data-orso-checks", "host");
  Object.assign(host.style, {
    position: "fixed",
    right: "12px",
    bottom: "12px",
    zIndex: "2147483646",
  });

  const shadow = host.attachShadow({ mode: "open" });
  const style = document.createElement("style");
  style.textContent = typeof __ORSO_CHECKS_CSS__ === "string" ? __ORSO_CHECKS_CSS__ : "";
  shadow.append(style);

  const mountEl = document.createElement("div");
  mountEl.className = "orso-checks-root";
  shadow.append(mountEl);
  document.documentElement.append(host);

  createApp(ChecksPanel).mount(mountEl);
}

if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", mount, { once: true });
  } else {
    mount();
  }
}
