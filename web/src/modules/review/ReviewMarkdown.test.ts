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
});
