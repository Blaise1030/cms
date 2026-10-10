// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import type { PrStatus } from "@/core/types";
import PrStatusPill from "./PrStatusPill.vue";
import { prStatusLabel } from "./prStatus";
import { useChangesStore } from "./store";

const base: PrStatus = {
  number: 1,
  url: "https://github.com/acme/site/pull/1",
  state: "draft",
  checks: "pending",
  previewUrl: null,
};

function mountWith(status: PrStatus | null) {
  setActivePinia(createPinia());
  useChangesStore().status = status;
  return mount(PrStatusPill);
}

describe("prStatusLabel", () => {
  it("joins PR state and checks", () => {
    expect(prStatusLabel(base)).toBe("Draft · Checks pending");
    expect(prStatusLabel({ ...base, state: "open", checks: "passing" })).toBe(
      "Open · Checks passing",
    );
    expect(prStatusLabel({ ...base, state: "approved", checks: "failing" })).toBe(
      "Approved · Checks failing",
    );
  });

  it("omits checks when there are none, and for merged or closed PRs", () => {
    expect(prStatusLabel({ ...base, state: "open", checks: "none" })).toBe("Open");
    expect(prStatusLabel({ ...base, state: "merged", checks: "passing" })).toBe("Merged");
    expect(prStatusLabel({ ...base, state: "closed", checks: "failing" })).toBe("Closed");
  });
});

describe("PrStatusPill", () => {
  it("renders nothing without a PR", () => {
    const w = mountWith(null);
    expect(w.find("[data-testid='pr-status']").exists()).toBe(false);
    expect(w.find("[data-testid='preview-deploy-link']").exists()).toBe(false);
  });

  it("shows the status and no link before a preview is deployed", () => {
    const w = mountWith(base);
    expect(w.find("[data-testid='pr-status']").text()).toBe("Draft · Checks pending");
    expect(w.find("[data-testid='pr-status']").attributes("data-state")).toBe("draft");
    expect(w.find("[data-testid='preview-deploy-link']").exists()).toBe(false);
  });

  it("links the deployed preview in a new tab", () => {
    const w = mountWith({
      ...base,
      state: "open",
      checks: "passing",
      previewUrl: "https://lw.pages.dev",
    });
    const a = w.find("[data-testid='preview-deploy-link']");
    expect(a.attributes("href")).toBe("https://lw.pages.dev");
    expect(a.attributes("target")).toBe("_blank");
    expect(a.attributes("rel")).toContain("noopener");
  });

  it("never links a non-http preview URL", () => {
    for (const previewUrl of ["javascript:alert(1)", "data:text/html,x", "//evil.dev"]) {
      const w = mountWith({ ...base, previewUrl });
      expect(w.find("[data-testid='preview-deploy-link']").exists()).toBe(false);
    }
  });
});
