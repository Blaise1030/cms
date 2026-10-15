import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import { startChange, terminalCheckpoint } from "../fixtures/daemon";
import { expect, openPrMenu, test } from "../fixtures/workbench";

const BRANCH = "content/merged-post";
const FILE = "merged-post.md";
const CONTENT = "# Merged post\n\nShipped through review.\n";

function git(cwd: string, args: string[]): string {
  return execFileSync(
    "git",
    [
      "-c",
      "user.name=Grace",
      "-c",
      "user.email=g@example.com",
      "-c",
      "commit.gpgsign=false",
      "-c",
      "core.hooksPath=/dev/null",
      ...args,
    ],
    { cwd, encoding: "utf8" },
  ).trim();
}

/** What a reviewer merging the PR on GitHub does: merge the change into the remote's `main`. */
function mergeOnRemote(bareRemote: string, tmp: string, branch: string) {
  const reviewer = path.join(tmp, "reviewer");
  git(tmp, ["clone", bareRemote, reviewer]);
  git(reviewer, ["merge", "--no-ff", `origin/${branch}`, "-m", `Merge ${branch}`]);
  git(reviewer, ["push", "origin", "main"]);
}

test("merged change shows as merged; Clean up returns to updated main", async ({
  page,
  workbench,
}) => {
  const { origin, token, projectId, projectDir, bareRemote, tmp, github } = workbench;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  // A change with its own file, shared as a draft PR.
  await startChange(workbench, "New post: Merged post");
  await page.reload();
  await expect(page.getByTestId("active-change")).toHaveText("Merged post");
  await terminalCheckpoint(workbench, "edit the notes");
  fs.writeFileSync(path.join(projectDir, FILE), CONTENT);
  git(projectDir, ["add", FILE]);
  git(projectDir, ["commit", "-m", "checkpoint: merged post"]);

  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-menu")).toBeVisible({ timeout: 30_000 });
  await openPrMenu(page);
  await expect(page.getByTestId("pr-status")).toHaveText("Draft");
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("cleanup-change")).toHaveCount(0);

  // The PR is merged on GitHub.
  mergeOnRemote(bareRemote, tmp, BRANCH);
  github.setPrStatus({ draft: false, state: "closed", merged: true });
  await page.reload();

  await openPrMenu(page);
  const pill = page.getByTestId("pr-status");
  await expect(pill).toHaveText("Merged", { timeout: 30_000 });
  await page.keyboard.press("Escape");
  const cleanup = page.getByTestId("cleanup-change");
  await expect(cleanup).toBeVisible();
  await expect(cleanup).toHaveText(/Clean up/);

  await cleanup.click();

  // Back on updated main; the local branch is gone (the remote one is left alone).
  await expect(page.getByTestId("active-change")).toHaveText("main", { timeout: 30_000 });
  await expect(page.getByTestId("pr-status")).toHaveCount(0);
  await expect(page.getByTestId("cleanup-change")).toHaveCount(0);
  expect(git(projectDir, ["branch", "--show-current"])).toBe("main");
  expect(git(projectDir, ["branch", "--list", BRANCH])).toBe("");
  expect(fs.readFileSync(path.join(projectDir, FILE), "utf8")).toBe(CONTENT);
  expect(git(bareRemote, ["branch", "--list", BRANCH])).toContain(BRANCH);

  // The changes menu no longer lists it.
  await page.getByTestId("changes-menu").click();
  await expect(page.getByTestId("changes-menu-content")).toBeVisible();
  await expect(page.locator(`[data-testid="change-item"][data-branch="${BRANCH}"]`)).toHaveCount(0);
  await expect(page.getByText("No open changes")).toBeVisible();
});
