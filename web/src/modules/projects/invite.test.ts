// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { ApiError } from "@/core/api";
import { collaboratorsUrl, openErrorMessage } from "./invite";

describe("collaboratorsUrl", () => {
  it("builds the GitHub collaborators URL for an owner/repo", () => {
    expect(collaboratorsUrl("acme/site")).toBe("https://github.com/acme/site/settings/access");
  });

  it("returns null when the repo is unknown or malformed", () => {
    expect(collaboratorsUrl(null)).toBeNull();
    expect(collaboratorsUrl("")).toBeNull();
    expect(collaboratorsUrl("site")).toBeNull();
    expect(collaboratorsUrl("a/b/c")).toBeNull();
  });
});

describe("openErrorMessage", () => {
  it("maps a 403 to the plain ask-an-owner message", () => {
    expect(openErrorMessage(new ApiError("SANDBOX", "You don't have push access", 403))).toBe(
      "Ask an owner to add you to this site.",
    );
  });

  it("maps a clone failure that hides the repo to the same message", () => {
    expect(openErrorMessage(new Error("git clone failed: Repository not found"))).toBe(
      "Ask an owner to add you to this site.",
    );
  });

  it("keeps other errors as-is", () => {
    expect(openErrorMessage(new Error("clone boom"))).toBe("clone boom");
  });
});
