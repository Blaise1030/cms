import { describe, expect, it } from "vitest";
import type { PrStatus } from "@/core/types";
import { prStatusLabel } from "./prStatus";

const base: PrStatus = {
  number: 1,
  url: "https://github.com/acme/site/pull/1",
  state: "draft",
  checks: "pending",
  previewUrl: null,
};

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
