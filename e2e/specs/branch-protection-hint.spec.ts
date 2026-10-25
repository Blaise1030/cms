import { startChange, chatCheckpoint } from "../fixtures/daemon";
import { expect, test } from "../fixtures/orso";

async function startChangeAndCheckpoint(
  page: import("@playwright/test").Page,
  orso: import("../fixtures/daemon").orsoEnv,
) {
  const { origin, token, projectId } = orso;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  await startChange(orso, "New post: Launch week");
  await page.reload();
  await expect(page.getByTestId("active-change")).toHaveText("Launch week");

  await chatCheckpoint(orso, "edit the notes");
}

test("unprotected main: hint after share, dismissal survives reload", async ({
  page,
  orso,
}) => {
  const { github } = orso;
  github.setMainProtected(false);
  await startChangeAndCheckpoint(page, orso);

  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-menu")).toBeVisible({ timeout: 30_000 });

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
  const doc = await page.request.get(`${orso.origin}/api/docs/branch-protection`);
  expect(doc.status()).toBe(200);
  expect(await doc.text()).toContain("# Protect `main` so merges need review");

  await hint.getByTestId("protection-hint-dismiss").click();
  await expect(hint).toBeHidden();

  await page.reload();
  await expect(page.getByTestId("active-change")).toHaveText("Launch week", { timeout: 60_000 });
  await expect(page.getByTestId("protection-hint")).toBeHidden();

  // Sharing again on the same project does not bring the hint back.
  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-menu")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("share-preview")).toBeEnabled();
  await expect(page.getByTestId("protection-hint")).toBeHidden();
  // Dismissed: the daemon no longer asks GitHub at all.
  expect(github.protectionGets()).toBe(1);
});

test("protected main: no hint after share", async ({ page, orso }) => {
  orso.github.setMainProtected(true);
  await startChangeAndCheckpoint(page, orso);

  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-menu")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("share-preview")).toBeEnabled();
  await expect(page.getByTestId("protection-hint")).toBeHidden();
  expect(orso.github.protectionGets()).toBeGreaterThan(0);
});

test("main protected by a ruleset requiring PRs: no hint after share", async ({
  page,
  orso,
}) => {
  orso.github.setMainProtected(false);
  orso.github.setMainRulesetRequiresPr(true);
  await startChangeAndCheckpoint(page, orso);

  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-menu")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("share-preview")).toBeEnabled();
  await expect(page.getByTestId("protection-hint")).toBeHidden();
  expect(orso.github.protectionGets()).toBeGreaterThan(0);
});
