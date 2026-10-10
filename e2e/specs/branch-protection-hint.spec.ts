import { agentCheckpoint } from "../fixtures/daemon";
import { expect, test } from "../fixtures/workbench";

async function startChangeAndCheckpoint(
  page: import("@playwright/test").Page,
  workbench: import("../fixtures/daemon").WorkbenchEnv,
) {
  const { origin, token, projectId } = workbench;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  await page.getByTestId("changes-menu").click();
  await page.getByTestId("new-change").click();
  await page.getByLabel("Change title").fill("New post: Launch week");
  await page.getByRole("button", { name: "Create change" }).click();
  await expect(page.getByTestId("active-change")).toHaveText("Launch week");

  await agentCheckpoint(workbench, "edit the notes");
}

test("unprotected main: hint after share, dismissal survives reload", async ({
  page,
  workbench,
}) => {
  const { github } = workbench;
  github.setMainProtected(false);
  await startChangeAndCheckpoint(page, workbench);

  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-link")).toBeVisible({ timeout: 30_000 });

  const hint = page.getByTestId("protection-hint");
  await expect(hint).toBeVisible();
  await expect(hint).toContainText("Protect main so merges need review");
  await expect(hint.getByTestId("protection-hint-settings")).toHaveAttribute(
    "href",
    `${github.baseUrl}/acme/site/settings/branches`,
  );
  const docs = hint.getByTestId("protection-hint-docs");
  await expect(docs).toHaveAttribute("href", "/api/docs/branch-protection");
  // The guide ships with the daemon, so the link works before this doc reaches main.
  const doc = await page.request.get(`${workbench.origin}/api/docs/branch-protection`);
  expect(doc.status()).toBe(200);
  expect(await doc.text()).toContain("# Protect `main` so merges need review");

  await hint.getByTestId("protection-hint-dismiss").click();
  await expect(hint).toBeHidden();

  await page.reload();
  await expect(page.getByTestId("active-change")).toHaveText("Launch week", { timeout: 60_000 });
  await expect(page.getByTestId("protection-hint")).toBeHidden();

  // Sharing again on the same project does not bring the hint back.
  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-link")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("share-preview")).toBeEnabled();
  await expect(page.getByTestId("protection-hint")).toBeHidden();
  // Dismissed: the daemon no longer asks GitHub at all.
  expect(github.protectionGets()).toBe(1);
});

test("protected main: no hint after share", async ({ page, workbench }) => {
  workbench.github.setMainProtected(true);
  await startChangeAndCheckpoint(page, workbench);

  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-link")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("share-preview")).toBeEnabled();
  await expect(page.getByTestId("protection-hint")).toBeHidden();
  expect(workbench.github.protectionGets()).toBeGreaterThan(0);
});

test("main protected by a ruleset requiring PRs: no hint after share", async ({
  page,
  workbench,
}) => {
  workbench.github.setMainProtected(false);
  workbench.github.setMainRulesetRequiresPr(true);
  await startChangeAndCheckpoint(page, workbench);

  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-link")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("share-preview")).toBeEnabled();
  await expect(page.getByTestId("protection-hint")).toBeHidden();
  expect(workbench.github.protectionGets()).toBeGreaterThan(0);
});
