import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import { expect, test } from "../fixtures/workbench";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", ["-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", ...args], { cwd, encoding: "utf8", stdio: "pipe" }).trim();
}

function pushConflictingMain(tmp: string, bareRemote: string): string {
  const pusher = path.join(tmp, "conflict-pusher");
  git(tmp, ["clone", bareRemote, pusher]);
  fs.writeFileSync(path.join(pusher, "index.html"), "<h1>Latest headline</h1>\n");
  git(pusher, ["add", "index.html"]);
  git(pusher, ["-c", "user.name=Grace", "-c", "user.email=g@example.com", "commit", "-m", "latest headline"]);
  git(pusher, ["push", "origin", "main"]);
  return git(pusher, ["rev-parse", "HEAD"]);
}

test("writer resolves a conflicting page with a visible file choice", async ({ page, workbench }) => {
  const { origin, token, projectId, projectDir, bareRemote, tmp } = workbench;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await page.getByTestId("changes-menu").click();
  await page.getByTestId("new-change").click();
  await page.getByLabel("Change title").fill("New page: Home");
  await page.getByRole("button", { name: "Create change" }).click();
  await expect(page.getByTestId("active-change")).toHaveText("Home");

  fs.writeFileSync(path.join(projectDir, "index.html"), "<h1>Your headline</h1>\n");
  git(projectDir, ["add", "index.html"]);
  git(projectDir, ["commit", "-m", "your headline"]);
  const before = git(projectDir, ["rev-parse", "HEAD"]);
  const latest = pushConflictingMain(tmp, bareRemote);

  await page.getByTestId("get-latest").click();
  await expect(page.getByTestId("conflict-view")).toBeVisible();
  await expect(page.getByTestId("conflict-file")).toContainText("index.html");
  await expect(page.getByTestId("finish-conflict")).toBeDisabled();
  await page.reload();
  await expect(page.getByTestId("conflict-view")).toBeVisible();
  await page.getByTestId("keep-yours").click();
  await expect(page.getByTestId("finish-conflict")).toBeEnabled();
  await page.getByTestId("finish-conflict").click();
  await expect(page.getByTestId("conflict-view")).toHaveCount(0);
  const merged = git(projectDir, ["rev-parse", "HEAD"]);
  expect(git(projectDir, ["rev-list", "--parents", "-n", "1", merged])).toBe(`${merged} ${before} ${latest}`);
  expect(fs.readFileSync(path.join(projectDir, "index.html"), "utf8")).toBe("<h1>Your headline</h1>\n");
});

test("writer can abandon a conflict without moving the branch", async ({ page, workbench }) => {
  const { origin, token, projectId, projectDir, bareRemote, tmp } = workbench;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await page.getByTestId("changes-menu").click();
  await page.getByTestId("new-change").click();
  await page.getByLabel("Change title").fill("New page: Home");
  await page.getByRole("button", { name: "Create change" }).click();
  await expect(page.getByTestId("active-change")).toHaveText("Home");
  fs.writeFileSync(path.join(projectDir, "index.html"), "<h1>Your headline</h1>\n");
  git(projectDir, ["add", "index.html"]);
  git(projectDir, ["commit", "-m", "your headline"]);
  const before = git(projectDir, ["rev-parse", "HEAD"]);
  pushConflictingMain(tmp, bareRemote);
  await page.getByTestId("get-latest").click();
  await expect(page.getByTestId("conflict-view")).toBeVisible();
  await page.getByTestId("abandon-conflict").click();
  await page.getByTestId("confirm-abandon").click();
  await expect(page.getByTestId("conflict-view")).toHaveCount(0);
  expect(git(projectDir, ["rev-parse", "HEAD"])).toBe(before);
  expect(git(projectDir, ["status", "--porcelain", "--untracked-files=no"])).toBe("");
});
