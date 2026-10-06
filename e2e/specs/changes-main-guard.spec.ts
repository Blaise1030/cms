import { execFileSync } from "node:child_process";

import { api } from "../fixtures/daemon";
import { expect, test } from "../fixtures/workbench";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

async function sendChatPrompt(page: import("@playwright/test").Page, text: string) {
  const composer = page.getByTestId("composer-editor");
  await composer.click();
  await page.keyboard.type(text);
  await page.getByTestId("send").click();
}

test("GitHub project on main prompts Start a change, then runs on the new branch", async ({
  page,
  workbench,
}) => {
  const { origin, token, projectId, projectDir } = workbench;
  const mainBefore = git(projectDir, ["rev-parse", "main"]);
  expect(git(projectDir, ["branch", "--show-current"])).toBe("main");

  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("chat-panel")).toBeVisible();

  await sendChatPrompt(page, "edit the notes");

  const dialog = page.getByRole("dialog", { name: "Start a change first" });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Change title").fill("New post: Launch week");
  await dialog.getByRole("button", { name: "Create change" }).click();

  await expect(page.getByTestId("active-change")).toHaveText("Launch week");
  await expect
    .poll(() => git(projectDir, ["branch", "--show-current"]))
    .toBe("content/launch-week");

  await expect
    .poll(() => {
      try {
        return git(projectDir, ["log", "-1", "--format=%s", "content/launch-week"]);
      } catch {
        return "";
      }
    })
    .toMatch(/^checkpoint:/);

  expect(git(projectDir, ["rev-parse", "main"])).toBe(mainBefore);
  expect(() =>
    git(projectDir, [
      "merge-base",
      "--is-ancestor",
      git(projectDir, ["rev-parse", "content/launch-week"]),
      "main",
    ]),
  ).toThrow();
});

test("local-only project edits main with no Start a change dialog", async ({
  page,
  workbench,
}) => {
  const { origin, token } = workbench;

  let created: Response | undefined;
  for (let attempt = 0; attempt < 3; attempt++) {
    try {
      created = await api(origin, token, "POST", "/api/projects", { name: "Local Only" });
      if (created.ok) break;
    } catch {
      // Daemon can briefly drop connections right after the prior fixture tears down.
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  expect(created?.ok).toBeTruthy();
  const { id: localId } = (await created!.json()) as { id: string };

  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${localId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 120_000 });
  await expect(page.getByTestId("chat-panel")).toBeVisible();

  await sendChatPrompt(page, "edit the notes");

  await expect(page.getByRole("dialog", { name: "Start a change first" })).toHaveCount(0);
  await expect(page.getByTestId("user-text")).toHaveText("edit the notes");
  await expect(page.getByTestId("assistant-text")).toContainText("edited", { timeout: 60_000 });
});
