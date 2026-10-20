import { describe, expect, it } from "vitest";
import { attachmentMessage, splitAttachments } from "./attachments";

const png = "/Users/me/orso-projects/.orso/site/attachments/5d83a351365d7d18.png";
const pdf = "/Users/me/orso-projects/.orso/site/attachments/0123456789abcdef.pdf";

describe("attachments", () => {
  it("appends saved paths to the text, one per line", () => {
    expect(attachmentMessage("Test", [png])).toBe(`Test\n${png}`);
    expect(attachmentMessage("  ", [png, pdf])).toBe(`Please review the attached files:\n${png}\n${pdf}`);
    expect(attachmentMessage("Test", [])).toBe("Test");
  });

  it("escapes spaces so the agent reads each path as one argument", () => {
    expect(attachmentMessage("x", ["/My Projects/attachments/5d83a351365d7d18.png"])).toBe(
      "x\n/My\\ Projects/attachments/5d83a351365d7d18.png",
    );
  });

  it("splits the appended paths back out of a sent message", () => {
    expect(splitAttachments(attachmentMessage("Test", [png, pdf]))).toEqual({
      text: "Test",
      files: [
        { name: "5d83a351365d7d18.png", kind: "PNG", image: true },
        { name: "0123456789abcdef.pdf", kind: "PDF", image: false },
      ],
    });
    expect(splitAttachments(attachmentMessage("x", ["/My Projects/attachments/5d83a351365d7d18.png"])).files).toEqual([
      { name: "5d83a351365d7d18.png", kind: "PNG", image: true },
    ]);
  });

  it("drops the stand-in prompt of a message that was only attachments", () => {
    expect(splitAttachments(attachmentMessage("", [png])).text).toBe("");
  });

  it("leaves text without trailing attachment lines alone", () => {
    for (const text of [
      "plain",
      `see ${png} please`,
      `${png}\nthen do this`,
      "/p/.orso/site/attachments/4b78.png",
    ]) {
      expect(splitAttachments(text)).toEqual({ text, files: [] });
    }
  });
});
