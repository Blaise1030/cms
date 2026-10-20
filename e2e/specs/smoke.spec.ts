import { expect, test } from "../fixtures/orso";

test("opens a cloned project and shows file tree and preview", async ({ page, orso }) => {
  const { origin, token, projectId } = orso;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);

  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  await page.getByTestId("workspace-tab-code").click();
  await expect(page.getByTestId("files-panel")).toBeVisible();
  // @pierre/trees renders rows in a shadow root; locators pierce it, text assertions on the host don't.
  await expect(
    page.getByTestId("project-files-tree").locator('[data-item-path="package.json"]'),
  ).toBeVisible();
});
