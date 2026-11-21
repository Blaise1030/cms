// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";
import { projectPath, routes } from "@/router";
import {
  connectTabOrder,
  DEFAULT_TAB_ORDER,
  firstTab,
  loadTabOrder,
  saveTabOrder,
  TAB_ORDER_KEY,
  tabOrderChanged,
} from "./tabOrder";

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

describe("tab order kept by the daemon", () => {
  const store = (saved: string[] | null) => ({
    load: vi.fn(async () => saved),
    save: vi.fn(async (order: string[]) => ({ order })),
  });

  it("takes the daemon's order over a fresh origin's empty local copy", async () => {
    const before = tabOrderChanged.value;
    const daemon = store(["history", "code"]);
    await connectTabOrder(daemon);
    expect(loadTabOrder().slice(0, 2)).toEqual(["history", "code"]);
    expect(firstTab()).toBe("history");
    expect(tabOrderChanged.value).toBe(before + 1);
    expect(daemon.save).not.toHaveBeenCalled();
  });

  it("sends a local order up when the daemon has none yet", async () => {
    localStorage.setItem(TAB_ORDER_KEY, JSON.stringify(["design"]));
    const daemon = store(null);
    await connectTabOrder(daemon);
    expect(daemon.save).toHaveBeenCalledWith(loadTabOrder());
    expect(loadTabOrder()[0]).toBe("design");
  });

  it("saves new orders to the daemon as well as locally, and survives a daemon that fails", async () => {
    const daemon = store(null);
    await connectTabOrder(daemon);
    const order = [...DEFAULT_TAB_ORDER].reverse();
    saveTabOrder(order);
    expect(loadTabOrder()).toEqual(order);
    expect(daemon.save).toHaveBeenLastCalledWith(order);

    const broken = { load: vi.fn(async () => Promise.reject(new Error("offline"))), save: vi.fn() };
    await expect(connectTabOrder(broken)).resolves.toBeUndefined();
    expect(loadTabOrder()).toEqual(order);
  });
});
