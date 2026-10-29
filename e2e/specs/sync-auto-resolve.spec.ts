import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import { startChange } from "../fixtures/daemon";
import { expect, test } from "../fixtures/orso";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", ["-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", ...args], { cwd, encoding: "utf8", stdio: "pipe" });
}

function isAncestor(cwd: string, ancestor: string, descendant: string): boolean {
  try {
    execFileSync("git", ["merge-base", "--is-ancestor", ancestor, descendant], { cwd });
    return true;
  } catch {
    return false;
  }
}

function pushConflictingMain(tmp: string, bareRemote: string, file: string) {
  const pusher = path.join(tmp, "conflict-pusher");
  git(tmp, ["clone", bareRemote, pusher]);
  fs.writeFileSync(path.join(pusher, file), "<h1>Latest headline</h1>\n");
  git(pusher, ["add", file]);
  git(pusher, ["-c", "user.name=Grace", "-c", "user.email=g@example.com", "commit", "-m", "latest headline"]);
  git(pusher, ["push", "origin", "main"]);
}

const DONE = "Done. Your changes and the latest site are combined.";
const FORBIDDEN = /branch|commit|merge|conflict|worktree|checkout|clone|\bgit\b|origin|main|markers|npm|<<<<<<</i;

test("a colliding draft is combined by the sync chat and keeps both sides", async ({ page, orso }) => {
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

  await page.addInitScript(() => localStorage.setItem("orso.chat.provider", "fake-sync"));
  await page.goto(`${origin}/?t=${token}`);
  await startChange(orso, "New page: Home");
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("active-change")).toHaveText("Home");

  fs.writeFileSync(path.join(projectDir, file), "<h1>Your headline</h1>\n");
  git(projectDir, ["add", file]);
  git(projectDir, ["commit", "-m", "your headline"]);
  const branch = git(projectDir, ["branch", "--show-current"]).trim();
  const ours = git(projectDir, ["rev-parse", "HEAD"]).trim();
  pushConflictingMain(tmp, bareRemote, file);

  await page.getByTestId("get-latest").click();
  const panel = page.getByTestId("chat-panel");
  await expect(panel).toBeVisible({ timeout: 60_000 });
  await expect(panel.getByTestId("sync-status").last()).toHaveText(DONE, { timeout: 60_000 });

  // The draft now carries the latest side, landed as a merge of both changes.
  expect(git(projectDir, ["show", `${branch}:${file}`])).toBe("<h1>Latest headline</h1>\n");
  const landed = git(projectDir, ["rev-parse", branch]).trim();
  expect(git(projectDir, ["rev-list", "--parents", "-n", "1", landed]).split(" ")).toHaveLength(3);
  expect(isAncestor(projectDir, ours, landed)).toBe(true);

  // Every writer-visible line in the chat avoids the implementation's words.
  const copy = await panel.innerText();
  expect(copy).toContain(DONE);
  expect(copy).not.toMatch(FORBIDDEN);
});
