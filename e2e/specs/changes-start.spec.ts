import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import { expect, test } from "../fixtures/workbench";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

/** Simulate an agent turn settling: write a file, then commit via the checkpoint message form. */
function agentCheckpoint(projectDir: string, relPath: string, contents: string, summary: string) {
  const full = path.join(projectDir, relPath);
  fs.mkdirSync(path.dirname(full), { recursive: true });
  fs.writeFileSync(full, contents);
  git(projectDir, ["add", "-A"]);
  git(projectDir, [
    "-c",
    "commit.gpgsign=false",
    "-c",
    "core.hooksPath=/dev/null",
    "commit",
    "--no-verify",
    "-m",
    `checkpoint: ${summary}`,
  ]);
}

test("starts a change, checkpoints on the branch, suffixes collisions", async ({
  page,
  workbench,
}) => {
  const { origin, token, projectId, projectDir, bareRemote } = workbench;
  const bareHeadBefore = git(bareRemote, ["rev-parse", "refs/heads/main"]);

  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  await page.getByTestId("new-change").click();
  await page.getByLabel("Change title").fill("New post: Launch week");
  await page.getByRole("button", { name: "Create change" }).click();

  await expect(page.getByTestId("active-change")).toHaveText("Launch week");

  expect(git(projectDir, ["branch", "--show-current"])).toBe("content/launch-week");
  expect(git(projectDir, ["config", "--get", "branch.content/launch-week.description"])).toBe(
    "Launch week",
  );

  agentCheckpoint(projectDir, "notes.md", "launch notes\n", "add notes");

  const changeTip = git(projectDir, ["rev-parse", "content/launch-week"]);
  const mainTip = git(projectDir, ["rev-parse", "main"]);
  expect(changeTip).not.toBe(mainTip);
  expect(git(projectDir, ["log", "-1", "--format=%s", "content/launch-week"])).toBe(
    "checkpoint: add notes",
  );
  expect(() => git(projectDir, ["merge-base", "--is-ancestor", changeTip, "main"])).toThrow();
  expect(git(bareRemote, ["rev-parse", "refs/heads/main"])).toBe(bareHeadBefore);
  expect(() => git(bareRemote, ["rev-parse", "refs/heads/content/launch-week"])).toThrow();

  await page.getByTestId("new-change").click();
  await page.getByLabel("Change title").fill("New post: Launch week");
  await page.getByRole("button", { name: "Create change" }).click();

  await expect(page.getByTestId("active-change")).toHaveText("Launch week");
  expect(git(projectDir, ["branch", "--show-current"])).toBe("content/launch-week-2");
});
