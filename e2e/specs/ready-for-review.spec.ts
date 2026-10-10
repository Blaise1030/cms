import { agentCheckpoint } from "../fixtures/daemon";
import { expect, test } from "../fixtures/workbench";

test("writer marks a shared draft change ready for review", async ({ page, workbench }) => {
  const { origin, token, projectId, github } = workbench;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  await page.getByTestId("changes-menu").click();
  await page.getByTestId("new-change").click();
  await page.getByLabel("Change title").fill("New post: Launch week");
  await page.getByRole("button", { name: "Create change" }).click();
  await expect(page.getByTestId("active-change")).toHaveText("Launch week");
  await agentCheckpoint(workbench, "edit the notes");

  // Share → draft PR.
  await page.getByTestId("share-preview").click();
  const pill = page.getByTestId("pr-status");
  await expect(pill).toHaveText("Draft", { timeout: 30_000 });
  await expect(page.getByTestId("open-on-github")).toHaveCount(0);

  const ready = page.getByTestId("ready-for-review");
  await expect(ready).toBeVisible();
  await ready.click();

  // The daemon marked the PR ready through GitHub's GraphQL API, by node id.
  await expect.poll(() => github.readyForReviewCalls()).toEqual(["PR_kwStub1"]);

  await expect(pill).toHaveText("Open", { timeout: 30_000 });
  await expect(ready).toHaveCount(0);
  const open = page.getByTestId("open-on-github");
  await expect(open).toHaveText("Open on GitHub");
  await expect(open).toHaveAttribute("href", "https://github.com/acme/site/pull/1");
  await expect(open).toHaveAttribute("target", "_blank");
});
