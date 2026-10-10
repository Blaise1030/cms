import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import { expect, test } from "../fixtures/workbench";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", ["-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", ...args], { cwd, encoding: "utf8", stdio: "pipe" }).trim();
}

function pushConflictingMain(tmp: string, bareRemote: string, file = "index.html"): string {
  const pusher = path.join(tmp, "conflict-pusher");
  git(tmp, ["clone", bareRemote, pusher]);
  const latest = file === "index.html" ? "<h1>Latest headline</h1>\n" : '<section id="hero"><h1>Latest headline</h1></section>\n';
  fs.writeFileSync(path.join(pusher, file), latest);
  git(pusher, ["add", file]);
  git(pusher, ["-c", "user.name=Grace", "-c", "user.email=g@example.com", "commit", "-m", "latest headline"]);
  git(pusher, ["push", "origin", "main"]);
  return git(pusher, ["rev-parse", "HEAD"]);
}

test("writer resolves a conflicting page with a visible file choice", async ({ page, workbench }) => {
  const { origin, token, projectId, projectDir, bareRemote, tmp } = workbench;
  const hero = "src/content/sections/hero.mdx";
  fs.mkdirSync(path.join(projectDir, "src/content/sections"), { recursive: true });
  fs.writeFileSync(path.join(projectDir, hero), '<section id="hero"><h1>Base headline</h1></section>\n');
  const packageJson = JSON.parse(fs.readFileSync(path.join(projectDir, "package.json"), "utf8")) as { scripts: Record<string, string> };
  packageJson.scripts.build = "mkdir -p dist && cp src/content/sections/hero.mdx dist/index.html";
  fs.writeFileSync(path.join(projectDir, "package.json"), JSON.stringify(packageJson));
  git(projectDir, ["add", "package.json", hero]);
  git(projectDir, ["commit", "-m", "add static preview build"]);
  git(projectDir, ["push", "origin", "main"]);
  fs.mkdirSync(path.join(projectDir, "node_modules"), { recursive: true });
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await page.getByTestId("changes-menu").click();
  await page.getByTestId("new-change").click();
  await page.getByLabel("Change title").fill("New page: Home");
  await page.getByRole("button", { name: "Create change" }).click();
  await expect(page.getByTestId("active-change")).toHaveText("Home");

  fs.writeFileSync(path.join(projectDir, hero), '<section id="hero"><h1>Your headline</h1></section>\n');
  git(projectDir, ["add", hero]);
  git(projectDir, ["commit", "-m", "your headline"]);
  const before = git(projectDir, ["rev-parse", "HEAD"]);
  const latest = pushConflictingMain(tmp, bareRemote, hero);

  await page.getByTestId("get-latest").click();
  await expect(page.getByTestId("conflict-view")).toBeVisible();
  await expect(page.getByTestId("conflict-file")).toContainText("hero.mdx");
  await expect(page.frameLocator('iframe[title="yours preview"]').locator("h1")).toHaveText("Your headline");
  await expect(page.frameLocator('iframe[title="latest preview"]').locator("h1")).toHaveText("Latest headline");
  await expect(page.frameLocator('iframe[title="yours preview"]').locator("section#hero")).toHaveCSS("outline-style", "solid");
  await expect(page.getByTestId("finish-conflict")).toBeDisabled();
  await page.reload();
  await expect(page.getByTestId("conflict-view")).toBeVisible();
  await page.getByTestId("keep-yours").click();
  await expect(page.frameLocator('iframe[title="result preview"]').locator("h1")).toHaveText("Your headline");
  await expect(page.getByTestId("finish-conflict")).toBeEnabled();
  await page.getByTestId("finish-conflict").click();
  await expect(page.getByTestId("conflict-view")).toHaveCount(0);
  const merged = git(projectDir, ["rev-parse", "HEAD"]);
  expect(git(projectDir, ["rev-list", "--parents", "-n", "1", merged])).toBe(`${merged} ${before} ${latest}`);
  expect(fs.readFileSync(path.join(projectDir, hero), "utf8")).toBe('<section id="hero"><h1>Your headline</h1></section>\n');
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
