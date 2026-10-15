import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import { startChange, terminalCheckpoint } from "../fixtures/daemon";
import { expect, test } from "../fixtures/workbench";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

/** Distinct committed edit on the current change branch (after the agent checkpoint). */
function commitDistinctEdit(cwd: string, content: string) {
  fs.writeFileSync(path.join(cwd, "notes.txt"), content);
  git(cwd, ["add", "notes.txt"]);
  git(cwd, [
    "-c",
    "commit.gpgsign=false",
    "-c",
    "core.hooksPath=/dev/null",
    "commit",
    "-m",
    `edit: ${content.trim()}`,
  ]);
}

test("lists open changes newest first and switches working tree", async ({ page, workbench }) => {
  const { origin, token, projectId, projectDir } = workbench;

  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  // First change + distinct file edit.
  await startChange(workbench, "New post: First change");
  await page.reload();
  await expect(page.getByTestId("active-change")).toHaveText("First change");
  await terminalCheckpoint(workbench, "edit the notes");
  commitDistinctEdit(projectDir, "content-from-first-change\n");

  // Second (newer) change + different edit.
  await startChange(workbench, "New post: Second change");
  await page.reload();
  await expect(page.getByTestId("active-change")).toHaveText("Second change");
  await terminalCheckpoint(workbench, "edit the notes");
  commitDistinctEdit(projectDir, "content-from-second-change\n");

  // Menu: both listed, newest first.
  await page.getByTestId("changes-menu").click();
  const items = page.getByTestId("change-item");
  await expect(items).toHaveCount(2);
  await expect(items.nth(0)).toContainText("Second change");
  await expect(items.nth(1)).toContainText("First change");
  await expect(items.nth(0)).toContainText(/ago|just now|\d/i);
  await expect(items.nth(1)).toContainText(/ago|just now|\d/i);

  // Switch to the first-created change (second in the newest-first list).
  await items.nth(1).click();
  await expect(page.getByTestId("active-change")).toHaveText("First change");
  expect(git(projectDir, ["branch", "--show-current"])).toBe("content/first-change");

  // File tree + viewer reflect the switched branch content.
  await page.getByTestId("workspace-tab-code").click();
  await expect(page.getByTestId("files-panel")).toBeVisible();
  await page.locator('[data-item-path="notes.txt"]').click();
  await expect(page.getByTestId("file-content")).toContainText("content-from-first-change");
});
