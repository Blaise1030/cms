import { execFileSync } from "node:child_process";

import { api, startChange, terminalCheckpoint } from "../fixtures/daemon";
import { expect, test } from "../fixtures/orso";

function git(cwd: string, args: string[]): string {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

test("GitHub project on main refuses an agent session, then runs on the new branch", async ({
  page,
  orso,
}) => {
  const { origin, token, projectId, projectDir } = orso;
  const mainBefore = git(projectDir, ["rev-parse", "main"]);
  expect(git(projectDir, ["branch", "--show-current"])).toBe("main");

  const refused = await api(origin, token, "POST", "/api/terminal", { project: projectId, provider: "fake" });
  expect(refused.status).toBe(409);
  const body = await refused.text();
  expect(body).toContain("CHANGE_REQUIRED");
  expect(body).toContain("Start a change first");

  await startChange(orso, "New post: Launch week");
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("active-change")).toHaveText("Launch week", { timeout: 60_000 });
  await expect
    .poll(() => git(projectDir, ["branch", "--show-current"]))
    .toBe("content/launch-week");

  await terminalCheckpoint(orso, "edit the notes");

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
  orso,
}) => {
  const { origin, token } = orso;

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

  await terminalCheckpoint({ origin, token, projectId: localId }, "edit the notes");

  await expect(page.getByRole("dialog", { name: "Start a change first" })).toHaveCount(0);
  await page.goto(`${origin}/p/${localId}/history`);
  await expect(page.getByTestId("checkpoint-item").first()).toContainText("the notes", {
    timeout: 60_000,
  });
});
