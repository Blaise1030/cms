// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { h } from "vue";
import UApp from "@nuxt/ui/components/App.vue";
import PageDetailsSidebar from "./PageDetailsSidebar.vue";

function render() {
  return mount(UApp, {
    slots: {
      default: () =>
        h(PageDetailsSidebar, {
          meta: null,
          findings: [],
          resources: { scripts: 15, styles: 7, images: 40, htmlBytes: 482_611 },
          path: "/",
          pageUrl: "http://127.0.0.1:4321/",
        }),
    },
  });
}

describe("PageDetailsSidebar", () => {
  it("names each resource count for hover and screen readers", () => {
    const w = render();
    const label = (key: string) => w.get(`[data-testid='page-details-stat-${key}']`).attributes("aria-label");
    expect(label("scripts")).toBe("Scripts: 15");
    expect(label("styles")).toBe("Styles: 7");
    expect(label("images")).toBe("Images: 40");
    expect(label("html")).toMatch(/^HTML size: /);
    // Focusable, so the tooltip also opens from the keyboard.
    expect(w.get("[data-testid='page-details-stat-scripts']").attributes("tabindex")).toBe("0");
  });
});
