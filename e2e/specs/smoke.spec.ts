import { expect, test } from "../fixtures/workbench";

test("opens a cloned project and shows file tree and preview", async ({ page, workbench }) => {
  const { origin, token, projectId } = workbench;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);

  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  await page
    .getByTestId("editor-menubar")
    .getByRole("menuitem", { name: "View", exact: true })
    .click();
  await page.getByRole("menuitemradio", { name: "Code" }).click();
  await expect(page.getByTestId("files-panel")).toBeVisible();
  await expect(page.getByRole("tree", { name: "Project files" })).toContainText("package.json");
});
