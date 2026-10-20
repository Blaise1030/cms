import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import { startChange } from "../fixtures/daemon";
import { expect, test } from "../fixtures/orso";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", ["-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", ...args], { cwd, encoding: "utf8", stdio: "pipe" });
}

function pushConflictingMain(tmp: string, bareRemote: string, file: string) {
  const pusher = path.join(tmp, "conflict-pusher");
  git(tmp, ["clone", bareRemote, pusher]);
  fs.writeFileSync(path.join(pusher, file), "<h1>Latest headline</h1>\n");
  git(pusher, ["add", file]);
  git(pusher, ["-c", "user.name=Grace", "-c", "user.email=g@example.com", "commit", "-m", "latest headline"]);
  git(pusher, ["push", "origin", "main"]);
}

test("syncing a colliding draft asks the writer one question and combines it", async ({ page, orso }) => {
  const { origin, token, projectId, projectDir, bareRemote, tmp } = orso;
  const file = "index.html";
  fs.writeFileSync(path.join(projectDir, file), "<h1>Base headline</h1>\n");
  const packageJson = JSON.parse(fs.readFileSync(path.join(projectDir, "package.json"), "utf8")) as { scripts: Record<string, string> };
  packageJson.scripts.build = `mkdir -p dist && cp ${file} dist/index.html`;
  fs.writeFileSync(path.join(projectDir, "package.json"), JSON.stringify(packageJson));
  git(projectDir, ["add", "package.json", file]);
  git(projectDir, ["commit", "-m", "add static preview build"]);
  git(projectDir, ["push", "origin", "main"]);
  fs.mkdirSync(path.join(projectDir, "node_modules"), { recursive: true });

  await page.addInitScript(() => localStorage.setItem("orso.terminal.provider", "fake-resolver"));
  await page.goto(`${origin}/?t=${token}`);
  await startChange(orso, "New page: Home");
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("active-change")).toHaveText("Home");

  fs.writeFileSync(path.join(projectDir, file), "<h1>Your headline</h1>\n");
  git(projectDir, ["add", file]);
  git(projectDir, ["commit", "-m", "your headline"]);
  const branch = git(projectDir, ["branch", "--show-current"]).trim();
  pushConflictingMain(tmp, bareRemote, file);

  await page.getByTestId("get-latest").click();
  const question = page.getByTestId("sync-question");
  await expect(question).toBeVisible({ timeout: 30_000 });
  await expect(question).toContainText("Which headline should stay?");
  await question.getByRole("radio", { name: "Theirs" }).check();
  await page.getByTestId("sync-continue").click();

  await expect(page.getByTestId("sync-done")).toBeVisible({ timeout: 30_000 });
  await page.getByTestId("sync-close").click();
  await expect(page.getByTestId("sync-done")).toHaveCount(0);

  expect(git(projectDir, ["show", `${branch}:${file}`])).toBe("Theirs\n");
  await expect(page.locator("body")).not.toContainText(/conflict|merge/i, { useInnerText: true });
});
