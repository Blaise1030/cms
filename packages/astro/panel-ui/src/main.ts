import { createApp } from "vue";
import ChecksPanel from "./ChecksPanel.vue";
import "./styles.css";

const ROOT_TEST_ID = "orso-checks-panel-host";

function mount() {
  document.querySelectorAll(`[data-testid="${ROOT_TEST_ID}"]`).forEach((el) => el.remove());
  document.querySelectorAll(`[data-testid="orso-checks-panel"]`).forEach((el) => {
    el.closest("[data-orso-checks]")?.remove();
    el.remove();
  });

  const host = document.createElement("div");
  host.dataset.testid = ROOT_TEST_ID;
  host.setAttribute("data-orso-checks", "host");
  Object.assign(host.style, {
    position: "fixed",
    right: "12px",
    bottom: "12px",
    zIndex: "2147483646",
  });
  document.documentElement.append(host);

  createApp(ChecksPanel).mount(host);
}

if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", mount, { once: true });
  } else {
    mount();
  }
}
