import { execFileSync } from "node:child_process";

import { startChange, terminalCheckpoint } from "../fixtures/daemon";
import { expect, openPrMenu, test } from "../fixtures/orso";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

async function startChangeAndCheckpoint(
  page: import("@playwright/test").Page,
  orso: import("../fixtures/daemon").orsoEnv,
  title = "New post: Launch week",
) {
  const { origin, token, projectId } = orso;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  await startChange(orso, title);
  await page.reload();
  await expect(page.getByTestId("active-change")).toHaveText("Launch week");

  await terminalCheckpoint(orso, "edit the notes");
}

test("shares a preview: push, draft PR, link; re-share advances remote without second PR", async ({
  page,
  orso,
}) => {
  const { projectDir, bareRemote, github } = orso;
  await startChangeAndCheckpoint(page, orso);

  await page.getByTestId("share-preview").click();

  await expect(page.getByTestId("pr-menu")).toBeVisible({ timeout: 30_000 });
  await openPrMenu(page);
  await expect(page.getByTestId("pr-link")).toHaveAttribute(
    "href",
    "https://github.com/acme/site/pull/1",
  );
  await page.keyboard.press("Escape");

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
  // Second append to `notes.md` (fake CLI `edit`); distinct from the first turn's text.
  await terminalCheckpoint(orso, "edit late");
  const localTip = git(projectDir, ["rev-parse", "content/launch-week"]);
  expect(localTip).not.toBe(remoteBefore);

  await page.getByTestId("share-preview").click();
  await expect
    .poll(() => git(bareRemote, ["rev-parse", "refs/heads/content/launch-week"]))
    .toBe(localTip);

  expect(github.pullsPosts()).toHaveLength(1);
  await expect(page.getByTestId("share-preview")).toBeEnabled();
  await openPrMenu(page);
  await expect(page.getByTestId("pr-link")).toHaveAttribute(
    "href",
    "https://github.com/acme/site/pull/1",
  );
});

test("share preview shows push-access error on 403", async ({ page, orso }) => {
  await startChangeAndCheckpoint(page, orso);
  orso.github.setPullsStatus(403);

  await page.getByTestId("share-preview").click();
  await expect(page.locator("[data-sonner-toast]")).toContainText("You don't have push access");
});

test("share preview prompts sign-in on 401", async ({ page, orso }) => {
  await startChangeAndCheckpoint(page, orso);
  orso.github.setPullsStatus(401);

  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("share-sign-in")).toBeVisible();
});
