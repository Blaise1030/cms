import { describe, expect, it } from "vitest";

import { isBridgeMessage } from "./frameBridge";

const element = {
  label: "h1#title",
  tag: '<h1 id="title">',
  selector: "body > h1",
  text: "Hello",
};

describe("isBridgeMessage", () => {
  it("accepts a picked element", () => {
    expect(isBridgeMessage({ type: "orso:preview", op: "pick", element })).toBe(true);
  });

  it("accepts a cancel", () => {
    expect(isBridgeMessage({ type: "orso:preview", op: "cancel" })).toBe(true);
    expect(isBridgeMessage({ type: "orso:preview", op: "fix", text: "Fix the title." })).toBe(true);
    expect(isBridgeMessage({ type: "orso:preview", op: "fix", text: "  " })).toBe(false);
    expect(isBridgeMessage({ type: "orso:preview", op: "fix" })).toBe(false);
  });

  it("accepts the page's own address, and only an http(s) URL", () => {
    const href = "http://127.0.0.1:56066/preview/site-1/blog/?q=1#top";
    expect(isBridgeMessage({ type: "orso:preview", op: "path", url: href })).toBe(true);
    expect(isBridgeMessage({ type: "orso:preview", op: "path" })).toBe(false);
    expect(isBridgeMessage({ type: "orso:preview", op: "path", url: "/blog/" })).toBe(false);
    expect(isBridgeMessage({ type: "orso:preview", op: "path", url: 3 })).toBe(false);
  });

  it("rejects unrelated messages", () => {
    expect(isBridgeMessage(null)).toBe(false);
    expect(isBridgeMessage({ type: "other", op: "pick", element })).toBe(false);
    expect(isBridgeMessage({ type: "orso:preview", op: "pick" })).toBe(false);
    expect(isBridgeMessage({ type: "orso:preview", op: "pick", element: {} })).toBe(false);
    expect(
      isBridgeMessage({ type: "orso:preview", op: "pick", element: { ...element, file: 3 } }),
    ).toBe(false);
    expect(isBridgeMessage({ type: "orso:preview", op: "scroll" })).toBe(false);
  });
});
