import { startChange, terminalCheckpoint } from "../fixtures/daemon";
import { expect, openPrMenu, test } from "../fixtures/workbench";

test("writer marks a shared draft change ready for review", async ({ page, workbench }) => {
  const { origin, token, projectId, github } = workbench;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  await startChange(workbench, "New post: Launch week");
  await page.reload();
  await expect(page.getByTestId("active-change")).toHaveText("Launch week");
  await terminalCheckpoint(workbench, "edit the notes");

  // Share → draft PR.
  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-menu")).toBeVisible({ timeout: 30_000 });
  await openPrMenu(page);
  const pill = page.getByTestId("pr-status");
  await expect(pill).toHaveText("Draft", { timeout: 30_000 });

  const ready = page.getByTestId("ready-for-review");
  await expect(ready).toBeVisible();
  await ready.click();

  // The daemon marked the PR ready through GitHub's GraphQL API, by node id.
  await expect.poll(() => github.readyForReviewCalls()).toEqual(["PR_kwStub1"]);

  await openPrMenu(page);
  await expect(pill).toHaveText("Open", { timeout: 30_000 });
  await expect(ready).toHaveCount(0);
  const open = page.getByTestId("pr-link");
  await expect(open).toHaveText("Open on GitHub");
  await expect(open).toHaveAttribute("href", "https://github.com/acme/site/pull/1");
  await expect(open).toHaveAttribute("target", "_blank");
});
