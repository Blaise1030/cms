import { expect, it } from "vitest";
import {
  clearStickyPageContext,
  formatPageContext,
  isLivePageContextTab,
  nextStickyPageContext,
  pageContextBadge,
  resolvePageContext,
  splitPageContext,
  withPageContext,
} from "./pageContext";

it("resolves preview path from page params", () => {
  expect(resolvePageContext({ tab: "preview", params: { path: "/about" } })).toEqual({
    tab: "preview",
    fields: { path: "/about" },
  });
});

it("defaults preview path to /", () => {
  expect(resolvePageContext({ tab: "preview", params: {} })).toEqual({
    tab: "preview",
    fields: { path: "/" },
  });
});

it("resolves content collection and entry", () => {
  expect(resolvePageContext({ tab: "content", params: { collection: "posts", entry: "hello" } })).toEqual({
    tab: "content",
    fields: { collection: "posts", entry: "hello" },
  });
});

it("returns null for tabs without a provider", () => {
  expect(resolvePageContext({ tab: "design", params: {} })).toBeNull();
  expect(resolvePageContext({ tab: "code", params: { path: "src/a.ts" } })).toBeNull();
});

it("formats a wire block the agent can read", () => {
  expect(formatPageContext({ tab: "preview", fields: { path: "/about" } })).toBe(
    "[page-context]\ntab: preview\npath: /about\n[/page-context]",
  );
});

it("appends context for send and strips it for display", () => {
  const ctx = { tab: "content" as const, fields: { collection: "posts", entry: "hello" } };
  const wired = withPageContext("Fix the hero", ctx);
  expect(wired.startsWith("Fix the hero\n")).toBe(true);
  expect(wired.endsWith("[/page-context]")).toBe(true);
  expect(splitPageContext(wired)).toEqual({ text: "Fix the hero", context: ctx });
});

it("leaves plain text alone when stripping", () => {
  expect(splitPageContext("Just a question")).toEqual({ text: "Just a question", context: null });
});

it("replaces an existing context block instead of stacking", () => {
  const first = withPageContext("Hi", { tab: "preview", fields: { path: "/a" } });
  const second = withPageContext(first, { tab: "preview", fields: { path: "/b" } });
  expect(splitPageContext(second)).toEqual({
    text: "Hi",
    context: { tab: "preview", fields: { path: "/b" } },
  });
});

it("builds a short composer/transcript badge label", () => {
  expect(pageContextBadge({ tab: "preview", fields: { path: "/about" } })).toBe("Preview · /about");
  expect(pageContextBadge({ tab: "content", fields: { collection: "posts", entry: "hello" } })).toBe(
    "Content · posts/hello",
  );
  expect(pageContextBadge({ tab: "content", fields: { collection: "posts" } })).toBe("Content · posts");
  expect(pageContextBadge({ tab: "content", fields: {} })).toBe("Content");
});

it("treats preview and content as live page-context tabs", () => {
  expect(isLivePageContextTab("preview")).toBe(true);
  expect(isLivePageContextTab("content")).toBe(true);
  expect(isLivePageContextTab("design")).toBe(false);
});

it("tracks live context on preview/content and freezes it on other modules", () => {
  const content = { tab: "content" as const, fields: { collection: "posts", entry: "hello" } };
  const live = nextStickyPageContext({ context: null, frozen: false }, "content", content);
  expect(live).toEqual({ context: content, frozen: false });

  const frozen = nextStickyPageContext(live, "design", null);
  expect(frozen).toEqual({ context: content, frozen: true });

  const preview = { tab: "preview" as const, fields: { path: "/about" } };
  expect(nextStickyPageContext(frozen, "preview", preview)).toEqual({ context: preview, frozen: false });
});

it("clears sticky context so another module injects nothing", () => {
  expect(clearStickyPageContext()).toEqual({ context: null, frozen: false });
});
