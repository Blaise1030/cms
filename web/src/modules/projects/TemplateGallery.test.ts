// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import TemplateGallery from "./TemplateGallery.vue";

describe("TemplateGallery", () => {
  it("offers the Astro template and asks the page to create", async () => {
    const w = mount(TemplateGallery);
    expect(w.text()).toContain("Create from a template");
    const card = w.get("[data-testid='template-astro-marketing']");
    expect(card.text()).toContain("Astro marketing");
    expect(card.text()).toContain("Astro landing page with blog and help content");
    await card.trigger("click");
    expect(w.emitted("create")).toHaveLength(1);
  });
});
