import { execFileSync } from "node:child_process";

import { agentCheckpoint } from "../fixtures/daemon";
import { expect, test } from "../fixtures/workbench";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

async function startChangeAndCheckpoint(
  page: import("@playwright/test").Page,
  workbench: import("../fixtures/daemon").WorkbenchEnv,
  title = "New post: Launch week",
) {
  const { origin, token, projectId } = workbench;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  await page.getByTestId("new-change").click();
  await page.getByLabel("Change title").fill(title);
  await page.getByRole("button", { name: "Create change" }).click();
  await expect(page.getByTestId("active-change")).toHaveText("Launch week");

  await agentCheckpoint(workbench, "edit the notes");
}

test("shares a preview: push, draft PR, link; re-share advances remote without second PR", async ({
  page,
  workbench,
}) => {
  const { projectDir, bareRemote, github } = workbench;
  await startChangeAndCheckpoint(page, workbench);

  await page.getByTestId("share-preview").click();

  await expect(page.getByTestId("pr-link")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("pr-link")).toHaveAttribute(
    "href",
    "https://github.com/acme/site/pull/1",
  );

  expect(git(bareRemote, ["rev-parse", "refs/heads/content/launch-week"])).toBe(
    git(projectDir, ["rev-parse", "content/launch-week"]),
  );

  const posts = github.pullsPosts();
  expect(posts).toHaveLength(1);
  expect(posts[0]!.path).toBe("/repos/acme/site/pulls");
  expect(posts[0]!.body).toMatchObject({
    draft: true,
    title: "Launch week",
    head: "content/launch-week",
    base: "main",
  });

  const remoteBefore = git(bareRemote, ["rev-parse", "refs/heads/content/launch-week"]);
  // `late` writes a new file (second `edit` would rewrite the same bytes → no checkpoint).
  await agentCheckpoint(workbench, "late");
  const localTip = git(projectDir, ["rev-parse", "content/launch-week"]);
  expect(localTip).not.toBe(remoteBefore);

  await page.getByTestId("share-preview").click();
  await expect
    .poll(() => git(bareRemote, ["rev-parse", "refs/heads/content/launch-week"]))
    .toBe(localTip);

  expect(github.pullsPosts()).toHaveLength(1);
  await expect(page.getByTestId("pr-link")).toHaveAttribute(
    "href",
    "https://github.com/acme/site/pull/1",
  );
});

test("share preview shows push-access error on 403", async ({ page, workbench }) => {
  await startChangeAndCheckpoint(page, workbench);
  workbench.github.setPullsStatus(403);

  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("changes-error")).toContainText("You don't have push access");
});

test("share preview prompts sign-in on 401", async ({ page, workbench }) => {
  await startChangeAndCheckpoint(page, workbench);
  workbench.github.setPullsStatus(401);

  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("share-sign-in")).toBeVisible();
});
