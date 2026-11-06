import { expect, test } from "../fixtures/orso";

test("preview back and forward move through the frame's history", async ({ page, orso }) => {
  const { origin, token, projectId } = orso;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await page.getByTestId("workspace-tab-preview").click();
  const address = page.getByTestId("preview-address");
  await expect(address).toHaveValue("/", { timeout: 120_000 });
  // Two navigations after the preview is up: the stack only holds pages from this document's life.
  await address.fill("/blog");
  await address.press("Enter");
  await expect(address).toHaveValue("/blog", { timeout: 60_000 });
  await address.fill("/help");
  await address.press("Enter");
  await expect(address).toHaveValue("/help", { timeout: 60_000 });
  await page.getByLabel("Back").click();
  await expect(address).toHaveValue("/blog", { timeout: 30_000 });
  await page.getByLabel("Forward").click();
  await expect(address).toHaveValue("/help", { timeout: 30_000 });
});
