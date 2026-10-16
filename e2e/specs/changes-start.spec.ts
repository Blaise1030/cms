import { execFileSync } from "node:child_process";

import { startChange, terminalCheckpoint } from "../fixtures/daemon";
import { expect, test } from "../fixtures/workbench";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
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

  await startChange(workbench, "New post: Launch week");
  await page.reload();

  await expect(page.getByTestId("active-change")).toHaveText("Launch week");

  expect(git(projectDir, ["branch", "--show-current"])).toBe("content/launch-week");
  expect(git(projectDir, ["config", "--get", "branch.content/launch-week.description"])).toBe(
    "Launch week",
  );

  // Fake CLI turn-end hook → git::checkpoint, not a raw git commit.
  await terminalCheckpoint(workbench, "edit the notes");

  const changeTip = git(projectDir, ["rev-parse", "content/launch-week"]);
  const mainTip = git(projectDir, ["rev-parse", "main"]);
  expect(changeTip).not.toBe(mainTip);
  expect(git(projectDir, ["log", "-1", "--format=%s", "content/launch-week"])).toMatch(
    /^checkpoint:/,
  );
  expect(() => git(projectDir, ["merge-base", "--is-ancestor", changeTip, "main"])).toThrow();
  expect(git(bareRemote, ["rev-parse", "refs/heads/main"])).toBe(bareHeadBefore);
  const remoteBranches = git(bareRemote, ["branch", "-a"]);
  expect(remoteBranches).not.toContain("content/launch-week");

  const health = await page.request.get(`${origin}/api/health`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  expect(health.ok()).toBeTruthy();

  await startChange(workbench, "New post: Launch week");
  await page.reload();

  await expect
    .poll(() => git(projectDir, ["branch", "--show-current"]))
    .toBe("content/launch-week-2");
  await expect(page.getByTestId("active-change")).toHaveText("Launch week");
});
