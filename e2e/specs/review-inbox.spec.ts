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
  github.addReviewPr({
    number: PR,
    title: "Fix the hero copy",
    author: "bob",
    headSha: sha,
    body: "Tightens the hero copy.",
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
  await item.click();
  await expect(page).toHaveURL(new RegExp(`/p/${projectId}/review/${PR}$`));
  await expect(page.getByTestId("review-title")).toHaveText("Fix the hero copy");
  await expect(page.getByText("What changed")).toBeVisible();
  await expect(page.getByText("serve.mjs").first()).toBeVisible({ timeout: 30_000 });
  return sha;
}

test("reviewer greenlights a change without touching the checkout", async ({ page, orso }) => {
  const { projectDir, github } = orso;
  const branchBefore = git(projectDir, ["branch", "--show-current"]);
  const statusBefore = git(projectDir, ["status", "--porcelain"]);

  const sha = await openReview(page, orso);
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
