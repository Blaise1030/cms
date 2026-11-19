// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";
import { projectPath, routes } from "@/router";
import { DEFAULT_TAB_ORDER, firstTab, loadTabOrder, saveTabOrder, TAB_ORDER_KEY } from "./tabOrder";

afterEach(() => localStorage.clear());

describe("tab order", () => {
  it("defaults to the built-in order", () => {
    expect(loadTabOrder()).toEqual(DEFAULT_TAB_ORDER);
    expect(firstTab()).toBe("preview");
  });

  it("keeps every tab once: saved first, unknown and repeated ids dropped, new tabs last", () => {
    localStorage.setItem(TAB_ORDER_KEY, JSON.stringify(["code", "gone", "code", "history"]));
    expect(loadTabOrder()).toEqual(["code", "history", "preview", "content", "design", "settings"]);
  });

  it("falls back to the default on a corrupt value", () => {
    localStorage.setItem(TAB_ORDER_KEY, "{not json");
    expect(loadTabOrder()).toEqual(DEFAULT_TAB_ORDER);
  });
});

describe("opening a project", () => {
  const router = () => createRouter({ history: createMemoryHistory(), routes });

  it("lands on the first tab in the saved order", async () => {
    saveTabOrder(["content", "preview", "design", "code", "history", "settings"]);
    expect(projectPath("site")).toBe("/p/site/content");

    const r = router();
    await r.push("/p/site");
    expect(r.currentRoute.value.path).toBe("/p/site/content");
  });

  it("still honours an explicit tab", () => {
    saveTabOrder(["content", "preview", "design", "code", "history", "settings"]);
    expect(projectPath("site", "code")).toBe("/p/site/code");
  });
});
