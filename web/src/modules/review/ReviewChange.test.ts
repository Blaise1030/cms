// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { defineComponent, h as el } from "vue";
import { TooltipProvider } from "reka-ui";
import { createMemoryHistory, createRouter } from "vue-router";
import { ApiError } from "@/core/api";
import type { ReviewDetail } from "@/core/types";
import { createFakeDaemon } from "@/test/fakeDaemon";

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
  const w = mount(Host, { global: { plugins: [router] }, attachTo: document.body });
  // The store loads the diff parser lazily.
  await vi.waitFor(() => expect(w.text()).not.toContain("Loading change"));
  await flushPromises();
  return { w, router };
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
      // Newest first, as the daemon sends it.
      items: [
        { kind: "state", at: "2026-01-06T00:00:00Z", state: "merged" },
        { kind: "review", author: "bo", at: "2026-01-05T00:00:00Z", verdict: "greenlit", body: null },
        { kind: "review", author: "dee", at: "2026-01-04T00:00:00Z", verdict: "commented", body: "Nit" },
        { kind: "review", author: "bo", at: "2026-01-03T00:00:00Z", verdict: "sent_back", body: "Fix the title" },
        { kind: "comment", id: 1, author: "cy", at: "2026-01-02T00:00:00Z", body: "Looks **close**", url: "u" },
        { kind: "push", at: "2026-01-01T01:00:00Z", commits: [
          { sha: "aaaaaaa", message: "First edit", author: "Ada" },
          { sha: "bbbbbbb", message: "Second edit", author: "Ada" },
        ] },
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

    it("renders each kind of timeline entry, newest first", async () => {
      const { w } = await mountChange({ ...base(), reviewConversation: vi.fn(async () => TALK) }, "conversation");
      const kinds = w.findAll("[data-testid='review-timeline'] > li").map((li) => li.attributes("data-kind"));
      expect(kinds).toEqual(["state", "review", "review", "review", "comment", "push"]);
      const push = w.get("[data-testid='review-timeline-push']");
      expect(push.text()).toContain("Ada");
      expect(push.text()).toContain("pushed 2 commits");
      expect(push.text()).toContain("aaaaaaa");
      expect(push.text()).toContain("Second edit");
      const comment = w.get("[data-testid='review-timeline-comment']");
      expect(comment.text()).toContain("cy");
      await vi.waitFor(() => expect(comment.find("strong").exists()).toBe(true));
      expect(comment.get("strong").text()).toBe("close");
      const reviews = w.findAll("[data-testid='review-timeline-review']");
      expect(reviews.map((r) => r.attributes("data-verdict"))).toEqual(["greenlit", "commented", "sent_back"]);
      expect(reviews[2]!.text()).toContain("bo sent this back");
      await vi.waitFor(() => expect(reviews[2]!.text()).toContain("Fix the title"));
      expect(reviews[0]!.text()).toContain("bo greenlit these changes");
      expect(w.get("[data-testid='review-timeline-state']").text()).toContain("Merged");
    });

    it("folds a long push to its first three commits and badges checkpoints", async () => {
      const commits = Array.from({ length: 5 }, (_, n) => ({
        sha: `c${n}00000`,
        message: n === 0 ? "checkpoint: Edit the hero" : `Edit ${n}`,
        author: "Ada",
      }));
      const talk = { ...TALK, items: [{ kind: "push", at: "2026-01-01T00:00:00Z", commits }] };
      const { w } = await mountChange({ ...base(), reviewConversation: vi.fn(async () => talk) }, "conversation");
      const push = () => w.get("[data-testid='review-timeline-push']");
      const shown = () => push().findAll("[data-testid='review-push-commit']");
      expect(push().text()).toContain("pushed 5 commits");
      expect(shown()).toHaveLength(3);
      // The checkpoint prefix is a badge, not text.
      expect(shown()[0]!.get("[data-testid='review-commit-checkpoint']").text()).toBe("checkpoint");
      expect(shown()[0]!.get("[data-testid='review-commit-subject']").text()).toBe("Edit the hero");
      expect(shown()[1]!.find("[data-testid='review-commit-checkpoint']").exists()).toBe(false);

      const more = push().get("[data-testid='review-push-more']");
      expect(more.text()).toBe("Show all 5 commits");
      expect(more.attributes("aria-expanded")).toBe("false");
      await more.trigger("click");
      await flushPromises();
      expect(shown()).toHaveLength(5);
      expect(push().get("[data-testid='review-push-more']").text()).toBe("Show fewer");
      expect(push().get("[data-testid='review-push-toggle']").attributes("aria-expanded")).toBe("true");

      // The header row toggles too.
      await push().get("[data-testid='review-push-header']").trigger("click");
      await flushPromises();
      expect(shown()).toHaveLength(3);
    });

    it("shows a short push in full with no toggle", async () => {
      const { w } = await mountChange({ ...base(), reviewConversation: vi.fn(async () => TALK) }, "conversation");
      const push = w.get("[data-testid='review-timeline-push']");
      expect(push.findAll("[data-testid='review-push-commit']")).toHaveLength(2);
      expect(push.find("[data-testid='review-push-more']").exists()).toBe(false);
      expect(push.find("[data-testid='review-push-toggle']").exists()).toBe(false);
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

    it("posts a comment and adds it to the timeline", async () => {
      const commentOnReview = vi.fn(async (_p: string, _n: number, body: string) => ({
        kind: "comment", id: 9, author: "me", at: new Date().toISOString(), body, url: "u9",
      }));
      const { w } = await mountChange({ ...base(), commentOnReview }, "conversation");
      const submit = () => w.get("[data-testid='review-comment-submit']");
      const input = w.get("[data-testid='review-comment-input']");
      expect(submit().attributes("disabled")).toBeDefined();
      await input.setValue("   ");
      expect(submit().attributes("disabled")).toBeDefined();
      await input.setValue("Ship it");
      expect(submit().attributes("disabled")).toBeUndefined();
      await w.get("[data-testid='review-comment-form']").trigger("submit");
      await flushPromises();
      expect(commentOnReview).toHaveBeenCalledWith("site", 7, "Ship it");
      const comments = w.findAll("[data-testid='review-timeline-comment']");
      expect(comments).toHaveLength(1);
      expect(comments[0]!.text()).toContain("me");
      expect((input.element as HTMLTextAreaElement).value).toBe("");
      expect(w.get("[data-testid='review-tab-conversation']").text()).toContain("1");
    });

    it("puts the comment box between the description and the timeline, and a new comment on top", async () => {
      const commentOnReview = vi.fn(async (_p: string, _n: number, body: string) => ({
        kind: "comment", id: 9, author: "me", at: new Date().toISOString(), body, url: "u9",
      }));
      const open = { ...DETAIL, open: true };
      const { w } = await mountChange(
        { ...base(), review: vi.fn(async () => open), reviewConversation: vi.fn(async () => TALK), commentOnReview },
        "conversation",
      );
      const ids = ["review-description", "review-comment-form", "review-timeline"];
      const els = ids.map((id) => w.get(`[data-testid='${id}']`).element);
      const follows = (a: Element, b: Element) => !!(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING);
      expect(follows(els[0]!, els[1]!)).toBe(true);
      expect(follows(els[1]!, els[2]!)).toBe(true);

      await w.get("[data-testid='review-comment-input']").setValue("Newest word");
      await w.get("[data-testid='review-comment-form']").trigger("submit");
      await flushPromises();
      const first = w.get("[data-testid='review-timeline'] > li");
      expect(first.attributes("data-kind")).toBe("comment");
      expect(first.text()).toContain("Newest word");
    });

    it("keeps the comment and says why when posting fails", async () => {
      const commentOnReview = vi.fn(async () => {
        throw new ApiError("UNKNOWN", "Couldn't post your comment. Try again in a moment.", 502);
      });
      const { w } = await mountChange({ ...base(), commentOnReview }, "conversation");
      const input = w.get("[data-testid='review-comment-input']");
      await input.setValue("Ship it");
      await w.get("[data-testid='review-comment-form']").trigger("submit");
      await flushPromises();
      expect(w.get("[data-testid='review-comment-error']").text()).toContain("Couldn't post your comment");
      expect((input.element as HTMLTextAreaElement).value).toBe("Ship it");
      expect(w.find("[data-testid='review-timeline-comment']").exists()).toBe(false);
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
        document.dispatchEvent(new Event("visibilitychange"));
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
      const { w } = await mountChange({ ...base(), review, decideReview });
      const { useReviewStore } = await import("./store");
      const store = useReviewStore();
      store.items = [{ ...DETAIL }];
      await w.get("[data-testid='review-greenlight']").trigger("click");
      await flushPromises();
      expect(decideReview).toHaveBeenCalledWith("site", 7, { decision: "greenlight", comment: undefined, headSha: "abc" });
      expect(store.items[0]?.myState).toBe("greenlit");
      expect(store.current?.myState).toBe("greenlit");
      expect(review).toHaveBeenCalledTimes(1);
    });

    it("says the change was updated and reloads detail and diff on head_moved", async () => {
      const decideReview = vi.fn(async () => {
        throw new ApiError("UNKNOWN", "This change was updated since you opened it", 409, { reason: "head_moved" });
      });
      const review = vi.fn(async () => ({ ...DETAIL }));
      const reviewDiff = vi.fn(async () => diffOf(DIFF));
      const { w } = await mountChange({ review, reviewDiff, decideReview });
      const { useReviewStore } = await import("./store");
      const store = useReviewStore();
      store.items = [{ ...DETAIL }];
      await w.get("[data-testid='review-greenlight']").trigger("click");
      await flushPromises();
      await vi.waitFor(() => expect(review).toHaveBeenCalledTimes(2));
      expect(reviewDiff).toHaveBeenCalledTimes(2);
      expect(w.text()).toContain("This change was updated since you opened it");
      expect(store.items[0]?.myState).toBe("new");
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
