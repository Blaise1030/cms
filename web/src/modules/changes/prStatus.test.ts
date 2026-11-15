import { describe, expect, it } from "vitest";
import type { PrStatus } from "@/core/types";
import { prStatusLabel } from "./prStatus";

const base: PrStatus = {
  number: 1,
  url: "https://github.com/acme/site/pull/1",
  state: "draft",
  checks: "pending",
  previewUrl: null,
  mergeable: null,
  mergeableState: "draft",
  upToDate: false,
};

describe("prStatusLabel", () => {
  it("joins PR state and checks", () => {
    expect(prStatusLabel(base)).toBe("Draft · Checks pending");
    expect(prStatusLabel({ ...base, state: "open", checks: "passing" })).toBe(
      "Waiting for greenlight · Checks passing",
    );
    expect(prStatusLabel({ ...base, state: "approved", checks: "failing" })).toBe(
      "Greenlit · Checks failing",
    );
  });

  it("omits checks when there are none, and for merged or closed PRs", () => {
    expect(prStatusLabel({ ...base, state: "open", checks: "none" })).toBe("Waiting for greenlight");
    expect(prStatusLabel({ ...base, state: "merged", checks: "passing" })).toBe("Published");
    expect(prStatusLabel({ ...base, state: "closed", checks: "failing" })).toBe("Closed");
  });
});
