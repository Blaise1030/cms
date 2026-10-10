import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import { agentCheckpoint, type WorkbenchEnv } from "../fixtures/daemon";
import { expect, test } from "../fixtures/workbench";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", args, { cwd, encoding: "utf8", stdio: "pipe" }).trim();
}

function isAncestor(cwd: string, ancestor: string, descendant: string): boolean {
  try {
    execFileSync("git", ["merge-base", "--is-ancestor", ancestor, descendant], { cwd });
    return true;
  } catch {
    return false;
  }
}

/** Someone else lands a new page on the remote `main` (no overlap with the changes). */
function pushToRemoteMain(env: WorkbenchEnv): string {
  const pusher = path.join(env.tmp, "pusher");
  execFileSync("git", ["clone", env.bareRemote, pusher], { stdio: "pipe" });
  fs.writeFileSync(path.join(pusher, "index.html"), "<h1>Fresh from main</h1>\n");
  git(pusher, ["add", "index.html"]);
  git(pusher, [
    "-c",
    "user.name=Grace",
    "-c",
    "user.email=g@example.com",
    "-c",
    "commit.gpgsign=false",
    "commit",
    "-m",
    "Add home page",
  ]);
  git(pusher, ["push", "origin", "main"]);
  return git(env.bareRemote, ["rev-parse", "refs/heads/main"]);
}

async function newChange(page: import("@playwright/test").Page, title: string, label: string) {
  await page.getByTestId("changes-menu").click();
  await page.getByTestId("new-change").click();
  await page.getByLabel("Change title").fill(title);
  await page.getByRole("button", { name: "Create change" }).click();
  await expect(page.getByTestId("active-change")).toHaveText(label);
}

test("get latest merges new main into every open change without force-pushing", async ({
  page,
  workbench,
}) => {
  const { origin, token, projectId, projectDir, bareRemote } = workbench;

  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  // First change: edit + share (pushes the branch to the remote).
  await newChange(page, "New post: First change", "First change");
  await agentCheckpoint(workbench, "edit the notes");
  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-link")).toBeVisible({ timeout: 30_000 });
  const sharedTip = git(bareRemote, ["rev-parse", "refs/heads/content/first-change"]);

  // Second change (active from here on).
  await newChange(page, "New post: Second change", "Second change");
  await agentCheckpoint(workbench, "edit the notes");

  const preview = page.frameLocator('[data-testid="preview-frame"]');
  await expect(preview.locator("body")).toContainText("Not found", { timeout: 30_000 });

  const remoteMain = pushToRemoteMain(workbench);

  await page.getByTestId("get-latest").click();
  await expect(page.locator("[data-sonner-toast]")).toContainText("Up to date");

  // Local main fast-forwarded to the remote.
  expect(git(projectDir, ["rev-parse", "main"])).toBe(remoteMain);
  // Both changes contain the new page, via a merge (not a rebase) on top of their own work.
  for (const branch of ["content/first-change", "content/second-change"]) {
    expect(git(projectDir, ["show", `${branch}:index.html`])).toContain("Fresh from main");
    expect(isAncestor(projectDir, remoteMain, branch)).toBe(true);
    expect(git(projectDir, ["rev-list", "--parents", "-n", "1", branch]).split(" ")).toHaveLength(3);
  }
  // Still on the active change; working tree clean.
  expect(git(projectDir, ["branch", "--show-current"])).toBe("content/second-change");
  expect(git(projectDir, ["status", "--porcelain", "--untracked-files=no"])).toBe("");

  // No force-push: the shared remote branch is untouched and the local branch builds on it.
  expect(git(bareRemote, ["rev-parse", "refs/heads/content/first-change"])).toBe(sharedTip);
  expect(isAncestor(projectDir, sharedTip, "content/first-change")).toBe(true);

  // The active preview shows the new main content.
  await expect(preview.locator("body")).toContainText("Fresh from main");
});
