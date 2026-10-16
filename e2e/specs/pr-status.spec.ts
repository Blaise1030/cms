import { startChange, terminalCheckpoint } from "../fixtures/daemon";
import { expect, openPrMenu, test } from "../fixtures/workbench";

test("browser bar shows PR state, checks and preview link; changes menu shows PR state", async ({
  page,
  workbench,
}) => {
  const { origin, token, projectId, github } = workbench;
  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });

  await startChange(workbench, "New post: Launch week");
  await page.reload();
  await expect(page.getByTestId("active-change")).toHaveText("Launch week");
  await terminalCheckpoint(workbench, "edit the notes");

  // No PR yet: no status pill.
  await expect(page.getByTestId("pr-status")).toHaveCount(0);

  // Draft PR, checks pending, nothing deployed.
  github.setPrStatus({ checkRuns: [{ status: "in_progress", conclusion: null }] });
  await page.getByTestId("share-preview").click();
  await expect(page.getByTestId("pr-menu")).toBeVisible({ timeout: 30_000 });

  await openPrMenu(page);
  const pill = page.getByTestId("pr-status");
  await expect(pill).toHaveText("Draft · Checks pending");
  await expect(page.getByTestId("preview-deploy-link")).toHaveCount(0);

  // PR opened, checks green, preview deployed for the head SHA.
  github.setPrStatus({
    draft: false,
    checkRuns: [{ status: "completed", conclusion: "success" }],
    deployment: { environment_url: "https://content-launch-week.site.pages.dev" },
  });
  await page.reload();
  await openPrMenu(page);
  await expect(pill).toHaveText("Open · Checks passing", { timeout: 30_000 });
  const link = page.getByTestId("preview-deploy-link");
  await expect(link).toHaveAttribute("href", "https://content-launch-week.site.pages.dev");
  await expect(link).toHaveAttribute("target", "_blank");

  // An approving review.
  github.setPrStatus({ reviews: [{ user: { login: "grace" }, state: "APPROVED" }] });
  await page.reload();
  await openPrMenu(page);
  await expect(pill).toHaveText("Approved · Checks passing", { timeout: 30_000 });
  await page.keyboard.press("Escape");

  // The changes menu shows each change's PR state.
  await page.getByTestId("changes-menu").click();
  const item = page.locator('[data-testid="change-item"][data-branch="content/launch-week"]');
  await expect(item.getByTestId("change-pr-state")).toHaveText("Approved");
});
