// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { defineComponent, h as el } from "vue";
import { TooltipProvider } from "reka-ui";
import { createMemoryHistory, createRouter } from "vue-router";
import { ApiError } from "@/core/api";
import type { ReviewDetail, ReviewItem } from "@/core/types";
import { createFakeDaemon } from "@/test/fakeDaemon";
import { queryPlugin } from "@/test/queryClient";

const h = vi.hoisted(() => ({ daemon: null as unknown }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("../checkpoints/DiffView.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    __esModule: true,
    default: defineComponent({
      props: { diff: { type: Object, required: true } },
      setup: (p) => () => h("pre", { "data-testid": "diff-stub" }, p.diff.name),
    }),
  };
});
vi.mock("../checkpoints/ChangedFilesTree.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    __esModule: true,
    default: defineComponent({
      props: { files: { type: Array, required: true } },
      setup: (p) => () =>
        h("ul", { "data-testid": "tree-stub" }, (p.files as { path: string }[]).map((f) => h("li", f.path))),
    }),
  };
});

import ReviewChange from "./ReviewChange.vue";
import { reviewKeys } from "./queries";

const DETAIL: ReviewDetail = {
  number: 7, title: "Fresh homepage hero", author: "Ada", url: "https://example.test/7",
  updatedAt: new Date().toISOString(), headSha: "abc", checks: "passing", myState: "new", mine: false,
  description: "Rewrote the hero copy.", open: true, draft: false, canDecide: true, reason: null,
};
const DIFF = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,1 +1,1 @@\n-x\n+y\n";
const BINARY = "Binary files a/site/logo.png and b/site/logo.png differ\n";
const diffOf = (text: string, headSha: string | null = null) => ({ text, headSha });

const emptyTalk = () => ({ items: [], checks: [], reviewers: [] });

async function mountChange(api: Record<string, unknown>, page: "conversation" | "files" = "files") {
  h.daemon = createFakeDaemon({
    liveSiteUrl: vi.fn(async () => "https://acme.pages.dev"),
    reviewPreview: vi.fn(async () => ({ state: "none", headSha: "abc" })),
    reviewConversation: vi.fn(async () => emptyTalk()),
    ...api,
  });
  setActivePinia(createPinia());
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/p/:id/review/:number?", name: "project-review", component: { render: () => null } },
      { path: "/p/:id/review/:number/files", name: "project-review-files", component: { render: () => null } },
    ],
  });
  await router.push(page === "files" ? "/p/site/review/7/files" : "/p/site/review/7");
  // Tooltips need the provider `UApp` gives the real app.
  const Host = defineComponent({
    setup: () => () =>
      el(TooltipProvider, null, { default: () => el(ReviewChange, { projectId: "site", number: 7, page }) }),
  });
  const { client, plugin } = queryPlugin();
  const w = mount(Host, { global: { plugins: [router, plugin] }, attachTo: document.body });
  // The store loads the diff parser lazily.
  await vi.waitFor(() => expect(w.text()).not.toContain("Loading change"));
  // The Conversation page loads its component lazily too.
  if (page === "conversation") await vi.waitFor(() => expect(w.find("[data-testid='review-conversation']").exists()).toBe(true));
  await flushPromises();
  return { w, router, client };
}

beforeEach(() => {
  localStorage.clear();
});
afterEach(() => vi.useRealTimers());

describe("ReviewChange", () => {
  it("shows the header and what changed", async () => {
    const { w } = await mountChange({
      review: vi.fn(async () => DETAIL),
      reviewDiff: vi.fn(async () => diffOf(DIFF)),
    });
    expect(w.text()).toContain("Fresh homepage hero");
    expect(w.text()).toContain("Ada");
    // Live site lives on the Review page's frame, not in the header.
    expect(w.find("[data-testid='review-live-site']").exists()).toBe(false);
    expect(w.text()).toContain("What changed");
    expect(w.get("[data-testid='tree-stub']").text()).toContain("a.txt");
    expect(w.get("[data-testid='diff-stub']").text()).toBe("a.txt");
  });

  it("puts the Preview / What changed switch in the top strip beside Refresh, on the Review page only", async () => {
    const files = await mountChange({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => diffOf(DIFF)) }, "files");
    const strip = files.w.get("[data-testid='review-pages']");
    const tabs = strip.get("[data-testid='review-view-tabs']");
    expect(tabs.text()).toContain("Preview");
    expect(tabs.text()).toContain("What changed");
    const refresh = strip.get("[data-testid='review-refresh']").element;
    expect(!!(tabs.element.compareDocumentPosition(refresh) & Node.DOCUMENT_POSITION_FOLLOWING)).toBe(true);
    // No second row under the strip.
    expect(files.w.get("[data-testid='review-files']").find("[data-testid='review-view-tabs']").exists()).toBe(false);
    files.w.unmount();

    const talk = await mountChange({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => diffOf(DIFF)) }, "conversation");
    expect(talk.w.find("[data-testid='review-view-tabs']").exists()).toBe(false);
    expect(talk.w.find("[data-testid='review-refresh']").exists()).toBe(true);
  });

  it("fills the view with the preview by default and switches to what changed", async () => {
    const { w } = await mountChange({
      review: vi.fn(async () => DETAIL),
      reviewDiff: vi.fn(async () => diffOf(DIFF)),
    });
    const shown = (id: string) => (w.get(`[data-testid='${id}']`).element as HTMLElement).style.display !== "none";
    const tabs = w.findAll("[data-testid='review-view-tabs'] [role='tab']");
    expect(tabs.map((t) => t.text())).toEqual(["Preview", "What changed"]);
    expect(w.get("[data-testid='review-preview']").classes()).toContain("flex-1");
    expect(shown("review-preview")).toBe(true);
    expect(shown("review-changes")).toBe(false);
    await tabs[1]!.trigger("mousedown", { button: 0 });
    await flushPromises();
    expect(shown("review-preview")).toBe(false);
    expect(shown("review-changes")).toBe(true);
    // The decision stays in reach whichever view is showing.
    expect(w.find("[data-testid='review-greenlight']").isVisible()).toBe(true);
    expect(w.find("[data-testid='review-send-back']").isVisible()).toBe(true);
  });

  it("lists a binary-only change", async () => {
    const { w } = await mountChange({
      review: vi.fn(async () => DETAIL),
      reviewDiff: vi.fn(async () => diffOf(BINARY)),
    });
    expect(w.get("[data-testid='tree-stub']").text()).toContain("site/logo.png");
    expect(w.text()).toContain("Binary file changed.");
  });

  it("shows the gone copy with a way back on 404", async () => {
    const { w, router } = await mountChange({
      review: vi.fn(async () => {
        throw new ApiError("UNKNOWN", "This change is no longer waiting", 404);
      }),
      reviewDiff: vi.fn(async () => diffOf(DIFF)),
    });
    expect(w.text()).toContain("This change is no longer waiting");
    await w.get("[data-testid='review-back']").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe("/p/site/review");
  });

  it("shows an error message when opening fails for another reason", async () => {
    const { w } = await mountChange({
      review: vi.fn(async () => {
        throw new ApiError("UNKNOWN", "GitHub is unreachable", 502);
      }),
      reviewDiff: vi.fn(async () => diffOf(DIFF)),
    });
    expect(w.get("[role='alert']").text()).toContain("GitHub is unreachable");
    expect(w.text()).not.toContain("no longer waiting");
  });

  describe("pages", () => {
    const base = () => ({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => diffOf(DIFF)) });

    it("opens on Conversation and links to Review, with counts", async () => {
      const { w, router } = await mountChange(
        {
          ...base(),
          reviewConversation: vi.fn(async () => ({
            items: [
              { kind: "comment", id: 1, author: "cy", at: "2026-01-01T00:00:00Z", body: "hi", url: "u" },
              { kind: "push", at: "2026-01-02T00:00:00Z", commits: [{ sha: "abc1234", message: "Edit", author: "Ada" }] },
            ],
            checks: [],
            reviewers: [],
          })),
        },
        "conversation",
      );
      const conv = w.get("[data-testid='review-tab-conversation']");
      const files = w.get("[data-testid='review-tab-files']");
      expect(conv.attributes("aria-current")).toBe("page");
      expect(files.attributes("aria-current")).toBeUndefined();
      expect(conv.text()).toContain("Conversation");
      expect(conv.text()).toContain("1");
      expect(files.text()).toContain("Review");
      expect(files.text()).toContain("1");
      expect(files.attributes("href")).toBe("/p/site/review/7/files");
      expect(w.find("[data-testid='review-conversation']").exists()).toBe(true);
      // One card holds the tabs and the page.
      expect(w.get("[data-testid='review-card']").find("[data-testid='review-pages']").exists()).toBe(true);
      expect(w.get("[data-testid='review-card']").find("[data-testid='review-conversation']").exists()).toBe(true);
      expect(w.find("[data-testid='review-files']").exists()).toBe(false);
      await files.trigger("click");
      await flushPromises();
      expect(router.currentRoute.value.name).toBe("project-review-files");
    });

    it("shows the header and its decisions on both pages", async () => {
      for (const page of ["conversation", "files"] as const) {
        const { w } = await mountChange(
          { ...base(), review: vi.fn(async () => ({ ...DETAIL, labels: [{ name: "orso", color: "0e8a16" }] })) },
          page,
        );
        expect(w.get("[data-testid='review-title']").text()).toBe("Fresh homepage hero");
        expect(w.get("[data-testid='review-number']").text()).toBe("#7");
        expect(w.get("[data-testid='review-state']").text()).toBe("Open");
        expect(w.get("[data-testid='review-header-label']").text()).toBe("orso");
        expect(w.find("[data-testid='review-greenlight']").exists()).toBe(true);
        expect(w.find("[data-testid='review-send-back']").exists()).toBe(true);
        expect(w.findAll("[data-testid='review-card']")).toHaveLength(1);
        expect(w.find("[data-testid='review-files']").exists()).toBe(page === "files");
        w.unmount();
      }
    });

    it("labels a merged or draft change", async () => {
      const { w } = await mountChange({ ...base(), review: vi.fn(async () => ({ ...DETAIL, merged: true, open: false })) }, "conversation");
      expect(w.get("[data-testid='review-state']").text()).toBe("Merged");
      w.unmount();
      const d = await mountChange({ ...base(), review: vi.fn(async () => ({ ...DETAIL, draft: true })) }, "conversation");
      expect(d.w.get("[data-testid='review-state']").text()).toBe("Draft");
    });
  });

  describe("refresh", () => {
    it("reloads detail, diff and the conversation in place, spinning and disabled meanwhile", async () => {
      let release!: () => void;
      const held = new Promise<void>((r) => (release = r));
      const review = vi
        .fn()
        .mockResolvedValueOnce({ ...DETAIL })
        .mockImplementationOnce(async () => {
          await held;
          return { ...DETAIL, title: "Fresher hero" };
        });
      const reviewDiff = vi
        .fn()
        .mockResolvedValue(diffOf(DIFF));
      const reviewConversation = vi.fn(async () => ({ items: [], checks: [], reviewers: [] }));
      const reviewPreview = vi.fn(async () => ({ state: "none", headSha: "abc" }));
      const { w } = await mountChange({ review, reviewDiff, reviewConversation, reviewPreview }, "conversation");
      const button = () => w.get("[data-testid='review-refresh']");
      expect(button().attributes("aria-label")).toBe("Refresh");
      const conversations = reviewConversation.mock.calls.length;
      const previews = reviewPreview.mock.calls.length;
      await button().trigger("click");
      await flushPromises();
      expect(button().attributes("disabled")).toBeDefined();
      expect(button().attributes("aria-busy")).toBe("true");
      expect(button().find(".animate-spin").exists()).toBe(true);
      // The page stays up while it reloads.
      expect(w.text()).not.toContain("Loading change");
      expect(w.get("[data-testid='review-title']").text()).toBe("Fresh homepage hero");
      release();
      await flushPromises();
      expect(review).toHaveBeenCalledTimes(2);
      expect(reviewDiff).toHaveBeenCalledTimes(2);
      expect(w.get("[data-testid='review-title']").text()).toBe("Fresher hero");
      expect(reviewConversation.mock.calls.length).toBeGreaterThan(conversations);
      // On Conversation the preview isn't re-read.
      expect(reviewPreview.mock.calls.length).toBe(previews);
      expect(button().attributes("disabled")).toBeUndefined();
      expect(button().find(".animate-spin").exists()).toBe(false);
    });

    it("re-reads the preview on the Review page and says when the reload failed", async () => {
      const review = vi
        .fn()
        .mockResolvedValueOnce({ ...DETAIL })
        .mockRejectedValueOnce(new ApiError("UNKNOWN", "GitHub is unreachable", 502));
      const reviewPreview = vi.fn(async () => ({ state: "none", headSha: "abc" }));
      const { w } = await mountChange({ review, reviewDiff: vi.fn(async () => diffOf(DIFF)), reviewPreview }, "files");
      const previews = reviewPreview.mock.calls.length;
      await w.get("[data-testid='review-refresh']").trigger("click");
      await flushPromises();
      expect(reviewPreview.mock.calls.length).toBe(previews + 1);
      expect(w.get("[data-testid='review-refresh-error']").text()).toContain("GitHub is unreachable");
      // What was on screen stays.
      expect(w.get("[data-testid='review-title']").text()).toBe("Fresh homepage hero");
    });
  });

  describe("conversation", () => {
    const base = () => ({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => diffOf(DIFF)) });
    const TALK = {
      // Oldest first, as the daemon sends it; one entry a day.
      items: [
        { kind: "push", at: "2026-01-01T12:00:00Z", commits: [
          { sha: "aaaaaaa", message: "checkpoint: First edit", author: "Ada" },
          { sha: "bbbbbbb", message: "Second edit", author: "Ada" },
        ] },
        { kind: "comment", id: 1, author: "cy", avatarUrl: "https://avatars.example/cy", at: "2026-01-02T12:00:00Z", body: "Looks **close**", url: "https://gh/c/1" },
        { kind: "review", author: "bo", at: "2026-01-03T12:00:00Z", verdict: "sent_back", body: "Fix the title" },
        { kind: "review", author: "dee", at: "2026-01-04T12:00:00Z", verdict: "commented", body: "Nit", url: "https://gh/r/2" },
        { kind: "review", author: "bo", at: "2026-01-05T12:00:00Z", verdict: "greenlit", body: null },
        { kind: "state", at: "2026-01-06T12:00:00Z", state: "merged" },
      ],
      checks: [
        { name: "build", status: "completed", conclusion: "success", url: "https://gh/runs/1" },
        { name: "deploy", status: "completed", conclusion: "failure", url: "https://gh/runs/2" },
      ],
      reviewers: [
        { login: "bo", state: "greenlit" },
        { login: "eve", state: "requested" },
      ],
    };
    const kindsOf = (w: Awaited<ReturnType<typeof mountChange>>["w"]) =>
      w.findAll("[data-testid='review-timeline'] [data-kind]").map((row) => row.attributes("data-kind"));
    const commentReply = (body: string, at = new Date().toISOString()) => ({
      kind: "comment", id: 9, author: "me", at, body, url: "u9",
    });

    it("reads oldest first, with a divider for each day, messages and compact events", async () => {
      const { w } = await mountChange({ ...base(), reviewConversation: vi.fn(async () => TALK) }, "conversation");
      expect(kindsOf(w).filter((k) => k !== "day")).toEqual(["push", "comment", "review", "comment", "review", "state"]);
      expect(w.findAll("[data-testid='review-day']")).toHaveLength(6);

      const messages = w.findAll("[data-testid='review-timeline-comment']");
      expect(messages.map((m) => m.get("[data-testid='review-message-header']").text())).toEqual([
        expect.stringContaining("cy"),
        expect.stringContaining("dee"),
      ]);
      expect(messages[0]!.get("img").attributes("src")).toBe("https://avatars.example/cy");
      await vi.waitFor(() => expect(messages[0]!.find("strong").exists()).toBe(true));
      expect(messages[0]!.get("strong").text()).toBe("close");
      expect(messages[1]!.get("[data-testid='review-message-open']").attributes("href")).toBe("https://gh/r/2");

      const reviews = w.findAll("[data-testid='review-timeline-review']");
      expect(reviews.map((r) => r.attributes("data-verdict"))).toEqual(["sent_back", "greenlit"]);
      expect(reviews[0]!.text()).toContain("bo sent this back");
      // The send-back's comment is quoted under it.
      await vi.waitFor(() => expect(reviews[0]!.get("[data-testid='review-verdict-comment']").text()).toContain("Fix the title"));
      expect(reviews[1]!.text()).toContain("bo greenlit this change");
      expect(w.get("[data-testid='review-timeline-state']").text()).toContain("Merged");
    });

    it("folds a push to one line until opened, badging checkpoints", async () => {
      const { w } = await mountChange({ ...base(), reviewConversation: vi.fn(async () => TALK) }, "conversation");
      const push = () => w.get("[data-testid='review-timeline-push']");
      expect(push().text()).toContain("Ada");
      expect(push().text()).toContain("pushed 2 commits");
      expect(push().findAll("[data-testid='review-push-commit']")).toHaveLength(0);
      const toggle = push().get("[data-testid='review-push-toggle']");
      expect(toggle.attributes("aria-expanded")).toBe("false");
      await toggle.trigger("click");
      await flushPromises();
      const shown = push().findAll("[data-testid='review-push-commit']");
      expect(shown).toHaveLength(2);
      expect(shown[0]!.get("[data-testid='review-commit-checkpoint']").text()).toBe("checkpoint");
      expect(shown[0]!.get("[data-testid='review-commit-subject']").text()).toBe("First edit");
      expect(shown[1]!.find("[data-testid='review-commit-checkpoint']").exists()).toBe(false);
    });

    it("groups one author's messages within five minutes under one header", async () => {
      const t = Date.parse("2026-01-02T12:00:00Z");
      const talk = {
        ...TALK,
        items: [
          { kind: "comment", id: 1, author: "cy", at: new Date(t).toISOString(), body: "One", url: "u1" },
          { kind: "comment", id: 2, author: "cy", at: new Date(t + 60_000).toISOString(), body: "Two", url: "u2" },
          { kind: "comment", id: 3, author: "cy", at: new Date(t + 10 * 60_000).toISOString(), body: "Three", url: "u3" },
        ],
      };
      const { w } = await mountChange({ ...base(), reviewConversation: vi.fn(async () => talk) }, "conversation");
      const messages = w.findAll("[data-testid='review-timeline-comment']");
      expect(messages.map((m) => m.attributes("data-grouped") ?? "")).toEqual(["", "true", ""]);
      expect(messages[1]!.find("img").exists()).toBe(false);
      expect(messages[1]!.find("[data-testid='review-message-header']").exists()).toBe(false);
      expect(messages[2]!.find("[data-testid='review-message-header']").exists()).toBe(true);
    });

    it("loads the repo's review images through the daemon", async () => {
      const shot = "https://github.com/acme/site/blob/abc1234/shots/7/2026-11-19/0f8fad5b-d9cb-469f-a165-70867728950e.png?raw=true";
      const talk = {
        ...TALK,
        items: [{ kind: "comment", id: 3, author: "cy", at: "2026-01-02T12:00:00Z", body: `![screenshot](${shot})`, url: "u" }],
      };
      const { w } = await mountChange(
        {
          ...base(),
          review: vi.fn(async () => ({ ...DETAIL, url: "https://github.com/acme/site/pull/7" })),
          reviewConversation: vi.fn(async () => talk),
          reviewAssetUrl: (p: string, sha: string, path: string) => `/api/projects/${p}/reviews/assets?path=${path}&sha=${sha}`,
        },
        "conversation",
      );
      const msg = w.get("[data-testid='review-timeline-comment']");
      await vi.waitFor(() => expect(msg.find("[data-testid='review-markdown'] img").exists()).toBe(true));
      expect(msg.get("[data-testid='review-markdown'] img").attributes("src")).toBe(
        "/api/projects/site/reviews/assets?path=shots/7/2026-11-19/0f8fad5b-d9cb-469f-a165-70867728950e.png&sha=abc1234",
      );
    });

    it("renders the description as markdown with raw HTML left inert", async () => {
      const description = "Hello **there**\n\n<script>window.__pwned = 1</script><img src=x onerror=\"window.__pwned=1\">";
      const { w } = await mountChange(
        { ...base(), review: vi.fn(async () => ({ ...DETAIL, description })) },
        "conversation",
      );
      const box = w.get("[data-testid='review-description']");
      await vi.waitFor(() => expect(box.find("strong").exists()).toBe(true));
      expect(box.find("script").exists()).toBe(false);
      expect(box.find("img").exists()).toBe(false);
      // Shown as text, never as elements or handlers.
      expect(box.findAll("*").some((el) => el.attributes("onerror") !== undefined)).toBe(false);
      expect(box.text()).toContain("<script>");
      expect((window as unknown as { __pwned?: number }).__pwned).toBeUndefined();
    });

    it("folds a long description to its first line, pinned above the feed", async () => {
      const description = `# Hero rewrite\n\n${"Long paragraph. ".repeat(40)}`;
      const { w } = await mountChange(
        { ...base(), review: vi.fn(async () => ({ ...DETAIL, description })), reviewConversation: vi.fn(async () => TALK) },
        "conversation",
      );
      const box = w.get("[data-testid='review-description']");
      const toggle = box.get("[data-testid='review-description-toggle']");
      expect(toggle.text()).toContain("Hero rewrite");
      expect(toggle.attributes("aria-expanded")).toBe("false");
      await toggle.trigger("click");
      await flushPromises();
      expect(toggle.attributes("aria-expanded")).toBe("true");
      await vi.waitFor(() => expect(box.find("h1").exists()).toBe(true));
      const order = ["review-description", "review-feed", "review-comment-form"].map((id) => w.get(`[data-testid='${id}']`).element);
      const follows = (a: Element, b: Element) => !!(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING);
      expect(follows(order[0]!, order[1]!)).toBe(true);
      expect(follows(order[1]!, order[2]!)).toBe(true);
    });

    it("shows labels, checks, the deploy preview and reviewers in the sidebar", async () => {
      const { w } = await mountChange(
        {
          ...base(),
          review: vi.fn(async () => ({ ...DETAIL, labels: [{ name: "orso", color: "ffffff" }] })),
          reviewConversation: vi.fn(async () => TALK),
          reviewPreview: vi.fn(async () => ({ state: "ready", headSha: "abc", url: "https://pr-7.acme.pages.dev/" })),
        },
        "conversation",
      );
      const label = w.get("[data-testid='review-label']").element as HTMLElement;
      expect(label.textContent).toBe("orso");
      expect(label.style.backgroundColor).toBe("#ffffff");
      expect(label.style.color).toBe("#1f2328");
      expect(w.get("[data-testid='review-checks-count']").text()).toBe("1/2");
      expect(w.find("[data-testid='review-checks-list']").exists()).toBe(false);
      await w.get("[data-testid='review-checks-toggle']").trigger("click");
      const links = w.findAll("[data-testid='review-checks-list'] a");
      expect(links.map((a) => [a.text(), a.attributes("href")])).toEqual([
        ["build", "https://gh/runs/1"],
        ["deploy", "https://gh/runs/2"],
      ]);
      expect(w.get("[data-testid='review-deploy-link']").attributes("href")).toBe("https://pr-7.acme.pages.dev/");
      expect(w.findAll("[data-testid='review-reviewer']").map((r) => r.text())).toEqual([
        "boGreenlit",
        "eveAwaiting review",
      ]);
    });

    it("sends with the button, puts the message last and scrolls to it", async () => {
      const commentOnReview = vi.fn(async (_p: string, _n: number, body: string) => commentReply(body));
      const { w } = await mountChange(
        { ...base(), reviewConversation: vi.fn(async () => TALK), commentOnReview },
        "conversation",
      );
      const submit = () => w.get("[data-testid='review-comment-submit']");
      const input = w.get("[data-testid='review-comment-input']");
      expect(input.attributes("placeholder")).toBe("Message…");
      expect(submit().attributes("disabled")).toBeDefined();
      await input.setValue("   ");
      expect(submit().attributes("disabled")).toBeDefined();
      await input.setValue("Ship it");
      expect(submit().attributes("disabled")).toBeUndefined();
      const feed = w.get("[data-testid='review-feed']").element as HTMLElement;
      Object.defineProperty(feed, "scrollHeight", { configurable: true, value: 900 });
      await w.get("[data-testid='review-comment-form'] form").trigger("submit");
      await flushPromises();
      expect(commentOnReview).toHaveBeenCalledWith("site", 7, "Ship it");
      const messages = w.findAll("[data-testid='review-timeline-comment']");
      expect(messages.at(-1)!.text()).toContain("Ship it");
      expect(kindsOf(w).at(-1)).toBe("comment");
      expect((input.element as HTMLTextAreaElement).value).toBe("");
      expect(feed.scrollTop).toBe(900);
      // cy, bo's send-back and dee's review, plus the new one.
      expect(w.get("[data-testid='review-tab-conversation']").text()).toContain("4");
    });

    it("sends on ⌘/Ctrl+Enter and keeps Enter alone for a new line", async () => {
      const commentOnReview = vi.fn(async (_p: string, _n: number, body: string) => commentReply(body));
      const { w } = await mountChange({ ...base(), commentOnReview }, "conversation");
      const input = w.get("[data-testid='review-comment-input']");
      await input.setValue("Line one");
      await input.trigger("keydown", { key: "Enter" });
      await flushPromises();
      expect(commentOnReview).not.toHaveBeenCalled();
      await input.trigger("keydown", { key: "Enter", metaKey: true });
      await flushPromises();
      expect(commentOnReview).toHaveBeenCalledWith("site", 7, "Line one");
      await input.setValue("Line two");
      await input.trigger("keydown", { key: "Enter", ctrlKey: true });
      await flushPromises();
      expect(commentOnReview).toHaveBeenLastCalledWith("site", 7, "Line two");
    });

    it("keeps your place when entries arrive while you read further up, and follows at the bottom", async () => {
      const more = (n: number) => ({
        ...TALK,
        items: [
          ...TALK.items,
          ...Array.from({ length: n }, (_, i) => ({
            kind: "comment", id: 50 + i, author: "eve", at: "2026-01-07T12:00:00Z", body: `Late ${i}`, url: `u${i}`,
          })),
        ],
      });
      const reviewConversation = vi.fn().mockResolvedValueOnce(TALK).mockResolvedValueOnce(more(1)).mockResolvedValue(more(2));
      const { w } = await mountChange({ ...base(), reviewConversation }, "conversation");
      const el = w.get("[data-testid='review-feed']").element as HTMLElement;
      Object.defineProperty(el, "scrollHeight", { configurable: true, value: 1000 });
      Object.defineProperty(el, "clientHeight", { configurable: true, value: 200 });

      // Reading further up: a new entry doesn't move the view.
      el.scrollTop = 100;
      await w.get("[data-testid='review-refresh']").trigger("click");
      await vi.waitFor(() => expect(w.findAll("[data-testid='review-timeline-comment']")).toHaveLength(3));
      await flushPromises();
      expect(el.scrollTop).toBe(100);

      // At the bottom: it follows.
      el.scrollTop = 800;
      await w.get("[data-testid='review-refresh']").trigger("click");
      await vi.waitFor(() => expect(w.findAll("[data-testid='review-timeline-comment']")).toHaveLength(4));
      await flushPromises();
      expect(el.scrollTop).toBe(1000);
    });

    describe("sending optimistically", () => {
      const deferred = <T,>() => {
        let resolve!: (v: T) => void;
        let reject!: (e: unknown) => void;
        const promise = new Promise<T>((res, rej) => ((resolve = res), (reject = rej)));
        return { promise, resolve, reject };
      };
      const send = async (w: Awaited<ReturnType<typeof mountChange>>["w"], body: string) => {
        await w.get("[data-testid='review-comment-input']").setValue(body);
        await w.get("[data-testid='review-comment-form'] form").trigger("submit");
      };
      const last = (w: Awaited<ReturnType<typeof mountChange>>["w"]) =>
        w.findAll("[data-testid='review-timeline-comment']").at(-1)!;

      it("shows the message at once as Sending…, clears the composer, then swaps in GitHub's copy in place", async () => {
        const reply = deferred<unknown>();
        const commentOnReview = vi.fn(() => reply.promise);
        const { w } = await mountChange({ ...base(), reviewConversation: vi.fn(async () => TALK), commentOnReview }, "conversation");
        const { useGithubStore } = await import("@/modules/github/store");
        useGithubStore().user = { login: "me", id: 1, name: null, avatarUrl: "https://avatars.example/me" };
        await send(w, "Ship **it**");
        await flushPromises();
        const row = last(w);
        expect(row.attributes("data-pending")).toBe("sending");
        expect(row.get("[data-testid='review-message-sending']").text()).toBe("Sending…");
        expect(row.text()).toContain("me");
        expect(row.get("img").attributes("src")).toBe("https://avatars.example/me");
        await vi.waitFor(() => expect(row.find("strong").exists()).toBe(true));
        expect(((w.get("[data-testid='review-comment-input']").element) as HTMLTextAreaElement).value).toBe("");

        reply.resolve({ kind: "comment", id: 77, author: "me", at: "2026-01-08T12:00:00Z", body: "Ship **it**", url: "https://gh/c/77" });
        await flushPromises();
        const after = last(w);
        expect(after.attributes("data-pending")).toBeUndefined();
        expect(after.find("[data-testid='review-message-sending']").exists()).toBe(false);
        // Same element: keyed by the client id, so it wasn't remounted.
        expect(after.element).toBe(row.element);
      });

      it("marks a failed message with Retry and Delete, without re-reading the inbox", async () => {
        const commentOnReview = vi
          .fn()
          .mockRejectedValueOnce(new ApiError("UNKNOWN", "GitHub is unreachable", 502))
          .mockResolvedValueOnce({ kind: "comment", id: 78, author: "me", at: "2026-01-08T12:00:00Z", body: "Again", url: "u78" });
        const { w, client } = await mountChange({ ...base(), reviewConversation: vi.fn(async () => TALK), commentOnReview }, "conversation");
        client.setQueryData(reviewKeys.inbox("site"), []);
        await send(w, "Again");
        await flushPromises();
        const failed = last(w);
        expect(failed.attributes("data-pending")).toBe("failed");
        expect(failed.get("[data-testid='review-message-failed']").text()).toContain("Couldn't send.");
        expect(client.getQueryState(reviewKeys.inbox("site"))?.isInvalidated).toBe(false);

        await failed.get("[data-testid='review-message-retry']").trigger("click");
        await flushPromises();
        expect(commentOnReview).toHaveBeenCalledTimes(2);
        expect(commentOnReview).toHaveBeenLastCalledWith("site", 7, "Again");
        expect(last(w).attributes("data-pending")).toBeUndefined();
        expect(last(w).text()).toContain("Again");
      });

      it("deletes a failed message without putting it back in the composer", async () => {
        const commentOnReview = vi.fn().mockRejectedValue(new ApiError("UNKNOWN", "nope", 502));
        const { w } = await mountChange({ ...base(), reviewConversation: vi.fn(async () => TALK), commentOnReview }, "conversation");
        await send(w, "Gone soon");
        await flushPromises();
        await last(w).get("[data-testid='review-message-delete']").trigger("click");
        await flushPromises();
        expect(w.text()).not.toContain("Gone soon");
        expect(((w.get("[data-testid='review-comment-input']").element) as HTMLTextAreaElement).value).toBe("");
      });

      it("keeps a message that is still sending through a re-read of the conversation", async () => {
        const reply = deferred<unknown>();
        const reviewConversation = vi.fn(async () => TALK);
        const { w } = await mountChange({ ...base(), reviewConversation, commentOnReview: vi.fn(() => reply.promise) }, "conversation");
        await send(w, "Hold on");
        await flushPromises();
        const reads = reviewConversation.mock.calls.length;
        await w.get("[data-testid='review-refresh']").trigger("click");
        await flushPromises();
        expect(reviewConversation.mock.calls.length).toBeGreaterThan(reads);
        expect(last(w).text()).toContain("Hold on");
        expect(last(w).attributes("data-pending")).toBe("sending");
      });

      it("sends quick messages one after another, in order", async () => {
        const first = deferred<unknown>();
        const commentOnReview = vi
          .fn()
          .mockImplementationOnce(() => first.promise)
          .mockResolvedValueOnce({ kind: "comment", id: 81, author: "me", at: "2026-01-08T12:01:00Z", body: "Two", url: "u81" });
        const { w } = await mountChange({ ...base(), reviewConversation: vi.fn(async () => TALK), commentOnReview }, "conversation");
        await send(w, "One");
        await send(w, "Two");
        await flushPromises();
        // The second waits for the first.
        expect(commentOnReview).toHaveBeenCalledTimes(1);
        const bodies = () => w.findAll("[data-testid='review-timeline-comment']").slice(-2).map((m) => m.text());
        expect(bodies()[0]).toContain("One");
        expect(bodies()[1]).toContain("Two");
        first.resolve({ kind: "comment", id: 80, author: "me", at: "2026-01-08T12:00:00Z", body: "One", url: "u80" });
        await flushPromises();
        await vi.waitFor(() => expect(commentOnReview).toHaveBeenCalledTimes(2));
        await flushPromises();
        expect(commentOnReview.mock.calls.map((c) => c[2])).toEqual(["One", "Two"]);
        expect(bodies()[0]).toContain("One");
        expect(bodies()[1]).toContain("Two");
        expect(w.findAll("[data-pending]")).toHaveLength(0);
      });
    });
  });

  describe("preview", () => {
    const base = () => ({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => diffOf(DIFF)) });
    const READY = {
      state: "ready",
      headSha: "abc",
      url: "https://pr-7.acme.pages.dev/",
      liveUrl: "https://acme.pages.dev/",
    };
    const frame = (w: Awaited<ReturnType<typeof mountChange>>["w"], key: string) =>
      w.find(`iframe[data-testid='browser-frame-iframe-${key}']`);

    const shown = (w: Awaited<ReturnType<typeof mountChange>>["w"], key: string) =>
      (w.get(`[data-testid='browser-frame-side-${key}']`).element as HTMLElement).style.display !== "none";
    const strip = (w: Awaited<ReturnType<typeof mountChange>>["w"], key: string) =>
      w.find(`[data-testid='browser-frame-strip-${key}']`);

    it("always shows the change and the live site side by side once deployed", async () => {
      const reviewPreview = vi.fn(async () => READY);
      const { w } = await mountChange({ ...base(), reviewPreview });
      expect(reviewPreview).toHaveBeenCalledWith("site", 7);
      expect(frame(w, "change").attributes("src")).toBe("https://pr-7.acme.pages.dev/");
      expect(frame(w, "live").attributes("src")).toBe("https://acme.pages.dev/");
      expect(shown(w, "change") && shown(w, "live")).toBe(true);
      // Another origin: its own scripts may run, never with orso's.
      expect(frame(w, "change").attributes("sandbox")).toBe("allow-scripts allow-same-origin allow-forms allow-popups");
      expect(w.find("[data-testid='review-compare']").exists()).toBe(false);
    });

    it("folds one side at a time and remembers it in this browser", async () => {
      const { w } = await mountChange({ ...base(), reviewPreview: vi.fn(async () => READY) });
      const hideChange = w.get("[data-testid='browser-frame-collapse-change']");
      expect(hideChange.attributes("aria-label")).toBe("Hide this change");
      expect(w.get("[data-testid='browser-frame-collapse-live']").attributes("aria-label")).toBe("Hide live site");
      await hideChange.trigger("click");
      await flushPromises();
      expect(shown(w, "change")).toBe(false);
      expect(strip(w, "change").text()).toContain("This change");
      expect(shown(w, "live")).toBe(true);
      expect(localStorage.getItem("orso.review.collapsed")).toBe("change");
      // Folding the other side unfolds this one.
      await w.get("[data-testid='browser-frame-collapse-live']").trigger("click");
      await flushPromises();
      expect(shown(w, "change")).toBe(true);
      expect(shown(w, "live")).toBe(false);
      expect(strip(w, "change").exists()).toBe(false);
      await w.get("[data-testid='browser-frame-expand-live']").trigger("click");
      await flushPromises();
      expect(shown(w, "live")).toBe(true);
      expect(localStorage.getItem("orso.review.collapsed")).toBeNull();
      w.unmount();
      localStorage.setItem("orso.review.collapsed", "live");
      const again = await mountChange({ ...base(), reviewPreview: vi.fn(async () => READY) });
      expect(shown(again.w, "live")).toBe(false);
      expect(shown(again.w, "change")).toBe(true);
    });

    it("says the deploy is running and checks again every 10 s until it is ready", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const reviewPreview = vi
        .fn()
        .mockResolvedValueOnce({ state: "deploying", headSha: "abc", liveUrl: "https://acme.pages.dev/" })
        .mockResolvedValue(READY);
      const { w } = await mountChange({ ...base(), reviewPreview });
      expect(w.get("[data-testid='review-preview-deploying']").text()).toContain("Deploying preview…");
      // Nothing to show yet: the change starts folded with its state; the live site is there.
      expect(strip(w, "change").text()).toContain("Deploying…");
      expect(shown(w, "live")).toBe(true);
      expect(frame(w, "live").exists()).toBe(true);
      expect(w.get("[data-testid='review-greenlight']").attributes("disabled")).toBeUndefined();
      await vi.advanceTimersByTimeAsync(5000);
      expect(reviewPreview).toHaveBeenCalledTimes(1);
      await vi.advanceTimersByTimeAsync(5500);
      await flushPromises();
      expect(reviewPreview).toHaveBeenCalledTimes(2);
      expect(frame(w, "change").attributes("src")).toBe("https://pr-7.acme.pages.dev/");
      // Once deployed it unfolds on its own.
      expect(shown(w, "change")).toBe(true);
      await vi.advanceTimersByTimeAsync(20_000);
      expect(reviewPreview).toHaveBeenCalledTimes(2);
    });

    it("waits for a hidden tab to come back before checking the deploy again", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const reviewPreview = vi
        .fn()
        .mockResolvedValueOnce({ state: "deploying", headSha: "abc" })
        .mockResolvedValue(READY);
      const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
      try {
        await mountChange({ ...base(), reviewPreview });
        await vi.advanceTimersByTimeAsync(30_000);
        expect(reviewPreview).toHaveBeenCalledTimes(1);
        visibility.mockReturnValue("visible");
        document.dispatchEvent(new Event("visibilitychange", { bubbles: true }));
        await flushPromises();
        expect(reviewPreview).toHaveBeenCalledTimes(2);
      } finally {
        visibility.mockRestore();
      }
    });

    it("links a failed deploy to the change's checks", async () => {
      const { w } = await mountChange({
        ...base(),
        reviewPreview: vi.fn(async () => ({ state: "failed", headSha: "abc", liveUrl: "https://acme.pages.dev/" })),
      });
      const failed = w.get("[data-testid='review-preview-failed']");
      expect(failed.text()).toContain("The preview deploy failed.");
      expect(w.get("[data-testid='review-preview-checks']").attributes("href")).toBe("https://example.test/7/checks");
    });

    it("explains a site without deploy previews and points at what changed", async () => {
      const { w } = await mountChange({
        ...base(),
        reviewPreview: vi.fn(async () => ({ state: "none", headSha: "abc" })),
        liveSiteUrl: vi.fn(async () => null),
      });
      expect(w.get("[data-testid='review-preview-none']").text()).toContain(
        "This site doesn't publish previews for changes.",
      );
      await w.get("[data-testid='review-preview-to-changes']").trigger("click");
      await flushPromises();
      expect((w.get("[data-testid='review-changes']").element as HTMLElement).style.display).not.toBe("none");
    });

    it("notes a missing live site on its own side", async () => {
      const { w } = await mountChange({
        ...base(),
        reviewPreview: vi.fn(async () => ({ state: "ready", headSha: "abc", url: "https://pr-7.acme.pages.dev/" })),
        liveSiteUrl: vi.fn(async () => null),
      });
      // Without a live site it starts folded, saying so; unfolded, it explains.
      expect(frame(w, "change").exists()).toBe(true);
      expect(shown(w, "live")).toBe(false);
      expect(strip(w, "live").text()).toContain("Not found");
      await w.get("[data-testid='browser-frame-expand-live']").trigger("click");
      await flushPromises();
      expect(shown(w, "live")).toBe(true);
      expect(w.get("[data-testid='review-live-missing']").text()).toContain("No live site found");
    });

    it("shows a load error with Try again", async () => {
      const reviewPreview = vi
        .fn()
        .mockRejectedValueOnce(new ApiError("UNKNOWN", "GitHub is unreachable", 502))
        .mockResolvedValue(READY);
      const { w } = await mountChange({ ...base(), reviewPreview });
      expect(w.get("[data-testid='review-preview-error']").text()).toContain("GitHub is unreachable");
      await w.get("[data-testid='review-preview-retry']").trigger("click");
      await flushPromises();
      expect(frame(w, "change").exists()).toBe(true);
    });

    it("stops checking on unmount", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const reviewPreview = vi.fn(async () => ({ state: "deploying", headSha: "abc" }));
      const { w } = await mountChange({ ...base(), reviewPreview });
      w.unmount();
      await vi.advanceTimersByTimeAsync(30_000);
      expect(reviewPreview).toHaveBeenCalledTimes(1);
    });
  });

  describe("decision", () => {
    const base = () => ({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => diffOf(DIFF)) });

    it("offers Give greenlight and Send back", async () => {
      const { w } = await mountChange(base());
      const green = w.get("[data-testid='review-greenlight']");
      const back = w.get("[data-testid='review-send-back']");
      expect(green.text()).toBe("Give greenlight");
      expect(back.text()).toBe("Send back");
      expect(green.attributes("disabled")).toBeUndefined();
      expect(back.attributes("disabled")).toBeUndefined();
    });

    it("disables both and shows the reason when the reviewer cannot decide", async () => {
      const { w } = await mountChange({
        ...base(),
        review: vi.fn(async () => ({ ...DETAIL, canDecide: false, reason: "This is your change" })),
      });
      expect(w.get("[data-testid='review-greenlight']").attributes("disabled")).toBeDefined();
      expect(w.get("[data-testid='review-send-back']").attributes("disabled")).toBeDefined();
      expect(w.get("[data-testid='review-reason']").text()).toBe("This is your change");
    });

    it("greenlights with the head the reviewer saw and updates the row without reloading", async () => {
      const decideReview = vi.fn(async () => undefined);
      const review = vi.fn(async () => ({ ...DETAIL }));
      const { w, client } = await mountChange({ ...base(), review, decideReview });
      client.setQueryData(reviewKeys.inbox("site"), [{ ...DETAIL }]);
      // The re-read after the decision answers with GitHub's new state.
      review.mockResolvedValue({ ...DETAIL, myState: "greenlit" });
      await w.get("[data-testid='review-greenlight']").trigger("click");
      await flushPromises();
      expect(decideReview).toHaveBeenCalledWith("site", 7, { decision: "greenlight", comment: undefined, headSha: "abc" });
      expect(client.getQueryData<ReviewItem[]>(reviewKeys.inbox("site"))?.[0]?.myState).toBe("greenlit");
      // The change and the inbox are read again (marked stale) after the decision.
      await vi.waitFor(() => expect(review).toHaveBeenCalledTimes(2));
    });

    it("says the change was updated and reloads detail and diff on head_moved", async () => {
      const decideReview = vi.fn(async () => {
        throw new ApiError("UNKNOWN", "This change was updated since you opened it", 409, { reason: "head_moved" });
      });
      const review = vi.fn(async () => ({ ...DETAIL }));
      const reviewDiff = vi.fn(async () => diffOf(DIFF));
      const { w, client } = await mountChange({ review, reviewDiff, decideReview });
      client.setQueryData(reviewKeys.inbox("site"), [{ ...DETAIL }]);
      await w.get("[data-testid='review-greenlight']").trigger("click");
      await flushPromises();
      await vi.waitFor(() => expect(review).toHaveBeenCalledTimes(2));
      expect(reviewDiff).toHaveBeenCalledTimes(2);
      expect(w.text()).toContain("This change was updated since you opened it");
      expect(client.getQueryData<ReviewItem[]>(reviewKeys.inbox("site"))?.[0]?.myState).toBe("new");
    });

    describe("send back dialog", () => {
      const dialog = () => new DOMWrapper(document.body);
      async function openDialog(api: Record<string, unknown>) {
        const m = await mountChange({ review: vi.fn(async () => ({ ...DETAIL })), reviewDiff: vi.fn(async () => diffOf(DIFF)), ...api });
        await m.w.get("[data-testid='review-send-back']").trigger("click");
        await flushPromises();
        await dialog().get("[data-testid='send-back-comment']").setValue("  Fix the hero  ");
        return m;
      }
      async function submit() {
        await dialog().get("[role='dialog'] form").trigger("submit");
        await flushPromises();
      }
      afterEach(() => {
        document.body.innerHTML = "";
      });

      it("sends the trimmed comment and closes on success", async () => {
        const decideReview = vi.fn(async () => undefined);
        await openDialog({ decideReview });
        await submit();
        expect(decideReview).toHaveBeenCalledWith("site", 7, { decision: "send_back", comment: "Fix the hero", headSha: "abc" });
        expect(dialog().find("[data-testid='send-back-comment']").exists()).toBe(false);
      });

      it("closes on head_moved and shows the message on the page", async () => {
        const decideReview = vi.fn(async () => {
          throw new ApiError("UNKNOWN", "This change was updated since you opened it", 409, { reason: "head_moved" });
        });
        const { w } = await openDialog({ decideReview });
        await submit();
        await vi.waitFor(() => expect(dialog().find("[data-testid='send-back-comment']").exists()).toBe(false));
        expect(w.get("[data-testid='review-decide-error']").text()).toContain("This change was updated since you opened it");
      });

      it("keeps the dialog open and shows another failure inside it", async () => {
        const decideReview = vi.fn(async () => {
          throw new ApiError("UNKNOWN", "This is your change", 403);
        });
        const { w } = await openDialog({ decideReview });
        await submit();
        expect(dialog().get("[data-testid='send-back-error']").text()).toContain("This is your change");
        expect((dialog().get("[data-testid='send-back-comment']").element as HTMLTextAreaElement).value).toBe("  Fix the hero  ");
        expect(w.find("[data-testid='review-decide-error']").exists()).toBe(false);
      });
    });
  });
});
