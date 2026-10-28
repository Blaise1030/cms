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
