import { expect, test } from "../fixtures/workbench";

test("terminal shows the CLI and checkpoints a turn", async ({ page, workbench }) => {
  const { origin, token, projectId } = workbench;

  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  await page.getByTestId("changes-menu").click();
  await page.getByTestId("new-change").click();
  await page.getByLabel("Change title").fill("New post: Terminal e2e");
  await page.getByRole("button", { name: "Create change" }).click();
  await expect(page.getByTestId("active-change")).toHaveText("Terminal e2e");

  await page.getByTestId("terminal-provider").selectOption("fake");
  await page.getByTestId("terminal-start").click();

  await expect(page.locator(".xterm-rows")).toContainText("FAKE-CLI READY", { timeout: 60_000 });

  await page.locator(".xterm-screen").click();
  await page.keyboard.type("edit hello");
  await page.keyboard.press("Enter");
  await expect(page.locator(".xterm-rows")).toContainText("TURN-END", { timeout: 60_000 });

  await page.goto(`${origin}/p/${projectId}/history`);
  await expect(page.getByTestId("checkpoints-panel")).toBeVisible();
  await expect(page.getByTestId("checkpoint-item").first()).toContainText("hello", {
    timeout: 60_000,
  });
});

test("terminal leaves CLI output visible after exit", async ({ page, workbench }) => {
  const { origin, token, projectId } = workbench;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible();
  await page.getByTestId("changes-menu").click();
  await page.getByTestId("new-change").click();
  await page.getByLabel("Change title").fill("Terminal exit");
  await page.getByRole("button", { name: "Create change" }).click();
  await page.getByTestId("terminal-provider").selectOption("fake");
  await page.getByTestId("terminal-start").click();
  await expect(page.locator(".xterm-rows")).toContainText("FAKE-CLI READY");

  await page.locator(".xterm-screen").click();
  await page.keyboard.type("exit-test");
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("terminal-ended")).toContainText("Session ended (code 1)");
  await expect(page.locator(".xterm-rows")).toContainText("FAKE-CLI EXIT REASON");

  const panel = await page.getByTestId("terminal-panel").boundingBox();
  const ended = await page.getByTestId("terminal-ended").boundingBox();
  expect(panel).not.toBeNull();
  expect(ended).not.toBeNull();
  expect(ended!.height).toBeLessThan(panel!.height / 2);
  expect(ended!.y).toBeGreaterThan(panel!.y + panel!.height / 2);
});
