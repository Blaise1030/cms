// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import ReviewMarkdown from "./ReviewMarkdown.vue";

const IMAGE = "https://raw.githubusercontent.com/acme/site/abc/hero.png";

describe("ReviewMarkdown", () => {
  it("renders a trailing image instead of a streaming spinner", async () => {
    const w = mount(ReviewMarkdown, { props: { content: `Hero section:\n\n![hero](${IMAGE})` } });
    await flushPromises();
    await new Promise((r) => setTimeout(r, 50));
    await flushPromises();
    expect(w.find(`img[src="${IMAGE}"]`).exists()).toBe(true);
  });

  it("hides HTML comments but keeps them inside code", async () => {
    const w = mount(ReviewMarkdown, {
      props: { content: "<!-- orso-site-preview-deploy -->\nPreview is ready. Tag with `<!-- x -->`." },
    });
    await flushPromises();
    await new Promise((r) => setTimeout(r, 50));
    expect(w.text()).not.toContain("orso-site-preview-deploy");
    expect(w.text()).toContain("Preview is ready.");
    expect(w.get("code").text()).toBe("<!-- x -->");
  });
});

describe("ReviewMarkdown image grid", () => {
  it("lays neighbouring images out as a grid of links and drops unsafe ones", async () => {
    const w = mount(ReviewMarkdown, {
      props: { content: "Test images\n\n![a](https://x.test/a.png)\n![b](/api/b.png)\n![c](javascript:alert(1))" },
    });
    await flushPromises();
    const grid = w.get("[data-testid='review-image-grid']");
    expect(grid.findAll("img").map((i) => i.attributes("src"))).toEqual(["https://x.test/a.png", "/api/b.png"]);
    expect(grid.findAll("a").every((a) => a.attributes("target") === "_blank")).toBe(true);
    expect(w.text()).toContain("Test images");
  });
});
