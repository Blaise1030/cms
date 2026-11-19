import { execFileSync } from "node:child_process";

import { chatCheckpoint, pushPullHead, startChange } from "../fixtures/daemon";
import { expect, openPrMenu, test } from "../fixtures/orso";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

const PR = 3;

async function openReview(page: import("@playwright/test").Page, orso: import("../fixtures/daemon").orsoEnv) {
  const { origin, token, projectId, bareRemote, github } = orso;
  // A teammate's change, labelled for review, with its head where GitHub keeps it.
  const sha = pushPullHead(orso, PR, "serve.mjs", "// Fix the hero copy\n");
  // Its deploy published a preview, which the Preview pane frames; nothing builds locally.
  github.addSitePage("/pr-3/", "<h1>Fresh hero copy</h1>\n");
  github.setPrStatus({ deployment: { environment_url: `${github.baseUrl}/sites/pr-3/` } });
  github.addReviewPr({
    number: PR,
    title: "Fix the hero copy",
    author: "bob",
    headSha: sha,
    body: "Tightens the **hero** copy.",
    comments: [{ author: "carol", body: "Can we keep the old subtitle?" }],
  });
  expect(git(bareRemote, ["rev-parse", `refs/pull/${PR}/head`])).toBe(sha);

  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  await expect(page.getByTestId("review-badge")).toHaveText("1", { timeout: 30_000 });
  await page.getByTestId("workspace-tab-review").click();
  const item = page.getByTestId("review-item");
  await expect(item).toHaveCount(1);
  await expect(item).toContainText("Fix the hero copy");
  // The search narrows the list and stays in the URL.
  const search = page.getByTestId("review-search");
  await search.fill("is:open pricing");
  await expect(item).toHaveCount(0);
  await expect(page.getByTestId("review-empty-search")).toBeVisible();
  await expect(page).toHaveURL(/[?&]q=is(%3A|:)open(\+|%20)pricing/);
  await search.fill("is:open hero");
  await expect(item).toHaveCount(1);
  await item.click();
  // A change opens on its Conversation: description, the teammate's comment, a comment box.
  await expect(page).toHaveURL(new RegExp(`/p/${projectId}/review/${PR}$`));
  await expect(page.getByTestId("review-title")).toHaveText("Fix the hero copy");
  await expect(page.getByTestId("review-description").locator("strong")).toHaveText("hero");
  const comments = page.getByTestId("review-timeline-comment");
  await expect(comments).toHaveCount(1);
  await expect(comments.first()).toContainText("Can we keep the old subtitle?");
  await page.getByTestId("review-comment-input").fill("Yes, keeping it.");
  await page.getByTestId("review-comment-submit").click();
  await expect(comments).toHaveCount(2);
  // Oldest first, like a chat: the posted comment lands at the bottom.
  await expect(comments.first()).toContainText("Can we keep the old subtitle?");
  await expect(comments.last()).toContainText("Yes, keeping it.");
  expect(github.commentPosts()).toEqual([
    { path: `/repos/acme/site/issues/${PR}/comments`, body: { body: "Yes, keeping it." } },
  ]);

  // An attached image goes to the repo's orso-review-assets branch; the comment links it there.
  const png = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3]);
  await page.getByTestId("review-attach-input").setInputFiles({ name: "hero.png", mimeType: "image/png", buffer: png });
  await expect(page.getByTestId("review-attachment")).toHaveCount(1);
  await page.getByTestId("review-comment-input").fill("Here's the hero");
  await page.getByTestId("review-comment-submit").click();
  await expect(comments).toHaveCount(3);
  await expect(comments.last()).not.toHaveAttribute("data-pending", /.*/);
  const [upload] = github.gitCommits();
  expect(upload!.parents).toEqual([]);
  expect(upload!.files).toHaveLength(1);
  expect(upload!.files[0]).toMatch(new RegExp(`^uploads/${PR}/\\d{4}-\\d{2}-\\d{2}/[0-9a-f-]{36}\\.png$`));
  expect(github.branchTip("orso-review-assets")).toBe(upload!.sha);
  const posted = github.commentPosts()[1]!.body.body!;
  expect(posted).toMatch(/^Here's the hero\n\n!\[image\]\(/);
  expect(posted).toContain(`/acme/site/blob/${upload!.sha}/${upload!.files[0]}?raw=true)`);

  // Review holds the preview and what changed.
  await page.getByTestId("review-tab-files").click();
  await expect(page).toHaveURL(new RegExp(`/p/${projectId}/review/${PR}/files$`));
  // The preview fills the view; What changed is the other tab.
  await expect(page.getByTestId("review-preview")).toBeVisible();
  await page.getByRole("tab", { name: "What changed" }).click();
  await expect(page.getByText("serve.mjs").first()).toBeVisible({ timeout: 30_000 });
  await page.getByRole("tab", { name: "Preview" }).click();
  return sha;
}

/** The change's deploy preview shows in the Preview pane; the stub has no live site, so that side is folded. */
async function expectPreview(page: import("@playwright/test").Page) {
  await expect(
    page.frameLocator('iframe[data-testid="browser-frame-iframe-change"]').locator("h1"),
  ).toHaveText("Fresh hero copy", { timeout: 30_000 });
  await expect(page.getByTestId("browser-frame-strip-live")).toContainText("Not found");
}

test("reviewer greenlights a change without touching the checkout", async ({ page, orso }) => {
  const { projectDir, github } = orso;
  const branchBefore = git(projectDir, ["branch", "--show-current"]);
  const statusBefore = git(projectDir, ["status", "--porcelain"]);

  const sha = await openReview(page, orso);
  await expectPreview(page);
  await page.getByRole("button", { name: "Give greenlight" }).click();

  await expect.poll(() => github.reviewPosts().length).toBe(1);
  const post = github.reviewPosts()[0]!;
  expect(post.path).toBe(`/repos/acme/site/pulls/${PR}/reviews`);
  expect(post.body.event).toBe("APPROVE");
  expect(post.body.commit_id).toBe(sha);

  expect(git(projectDir, ["branch", "--show-current"])).toBe(branchBefore);
  expect(git(projectDir, ["status", "--porcelain"])).toBe(statusBefore);
});

test("reviewer sends a change back and the author sees why", async ({ page, orso }) => {
  const { github } = orso;
  const sha = await openReview(page, orso);

  await page.getByRole("button", { name: "Send back" }).click();
  await page.getByTestId("send-back-comment").fill("Please fix the title");
  await page.getByTestId("send-back-submit").click();

  await expect.poll(() => github.reviewPosts().length).toBe(1);
  const post = github.reviewPosts()[0]!;
  expect(post.body.event).toBe("REQUEST_CHANGES");
  expect(post.body.body).toBe("Please fix the title");
  expect(post.body.commit_id).toBe(sha);

  // The author's own change: shared as a PR, then sent back by the reviewer.
  await page.goto(`${orso.origin}/p/${orso.projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });
  await startChange(orso, "New post: Launch week");
  await page.reload();
  await expect(page.getByTestId("active-change")).toHaveText("Launch week");
  await chatCheckpoint(orso, "edit the notes");
  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-menu")).toBeVisible({ timeout: 30_000 });

  github.setPrStatus({
    draft: false,
    reviews: [
      { user: { login: "bob" }, state: "CHANGES_REQUESTED", body: "Please fix the title", commit_id: sha },
    ],
  });
  await page.reload();
  await expect(page.getByTestId("pr-menu")).toBeVisible({ timeout: 30_000 });
  await openPrMenu(page);
  await expect(page.getByTestId("pr-status")).toHaveText("Sent back", { timeout: 30_000 });
  await expect(page.getByTestId("pr-sent-back-reviewer")).toContainText("bob");
  await expect(page.getByTestId("pr-sent-back-comment")).toHaveText("Please fix the title");
});

test("reviewer queues a screenshot comment and sends it with the image on the assets branch", async ({ page, orso }) => {
  const { github } = orso;
  // Screen sharing without a prompt: a canvas painted like a page stands in for the tab.
  await page.addInitScript(() => {
    navigator.mediaDevices.getDisplayMedia = async () => {
      const canvas = document.createElement("canvas");
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
      const ctx = canvas.getContext("2d")!;
      const paint = () => {
        ctx.fillStyle = "#2b6cb0";
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        requestAnimationFrame(paint);
      };
      paint();
      return canvas.captureStream(30);
    };
  });
  await openReview(page, orso);
  await expectPreview(page);

  await page.getByTestId("browser-frame-pick-change").click();
  const overlay = page.getByTestId("browser-frame-pick-overlay-change");
  const box = (await overlay.boundingBox())!;
  await page.mouse.move(box.x + 40, box.y + 40);
  await page.mouse.down();
  await page.mouse.move(box.x + 240, box.y + 160, { steps: 5 });
  await page.mouse.up();

  await expect(page.getByTestId("shot-note-image")).toBeVisible({ timeout: 15_000 });
  await page.getByTestId("shot-note-input").fill("Hero headline is cut off");
  await page.getByTestId("shot-note-add").click();
  await expect(page.getByTestId("shot-queue-open")).toHaveText("Queued (1)");

  await page.getByTestId("shot-queue-open").click();
  await expect(page.getByTestId("shot-queue-item")).toHaveCount(1);
  await page.getByTestId("shot-queue-send").click();
  await expect.poll(() => github.reviewPosts().length).toBe(1);
  await expect(page.getByTestId("shot-queue-open")).toHaveCount(0);

  // Opening the review already attached an image to a comment (uploads/); the screenshot is its own commit.
  const commit = github.gitCommits().find((c) => c.files.some((f) => f.startsWith("shots/")));
  expect(commit!.files).toHaveLength(1);
  expect(commit!.files[0]).toMatch(new RegExp(`^shots/${PR}/\\d{4}-\\d{2}-\\d{2}/[0-9a-f-]{36}\\.(png|jpg)$`));
  expect(github.branchTip("orso-review-assets")).toBe(commit!.sha);
  const review = github.reviewPosts()[0]!.body;
  expect(review.event).toBe("COMMENT");
  expect(review.body).toContain("### 1. Hero headline is cut off");
  expect(review.body).toContain("Page: `/` (change, desktop)");
  expect(review.body).toContain(`![screenshot](`);
  expect(review.body).toContain(`/acme/site/blob/${commit!.sha}/${commit!.files[0]}?raw=true)`);
});
