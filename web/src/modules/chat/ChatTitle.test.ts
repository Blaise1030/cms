// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ChatTitle from "./ChatTitle.vue";

describe("ChatTitle", () => {
  it("shows an editor selection as a file tag, never the raw reference", () => {
    const w = mount(ChatTitle, {
      props: { title: '[selection in site/src/content/blog/en/how-we-run-design-reviews.md: "Feedback comes' },
    });
    expect(w.get("[data-testid='chat-title-tag']").text()).toBe("how-we-run-design-reviews.md");
    expect(w.text()).not.toContain("[selection");
  });

  it("shows plain titles as they are", () => {
    const w = mount(ChatTitle, { props: { title: "We would need a download button" } });
    expect(w.find("[data-testid='chat-title-tag']").exists()).toBe(false);
    expect(w.text()).toBe("We would need a download button");
  });
});
