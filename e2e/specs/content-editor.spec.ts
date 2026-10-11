import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import type { Page } from "@playwright/test";

import { api } from "../fixtures/daemon";
import { expect, test } from "../fixtures/workbench";

/** A real 8x8 RGB PNG (decodable, so the daemon's byte inspection accepts it). */
const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAEUlEQVR4nGO4Y6OBFTEMLQkAb5lQAdZVdE8AAAAASUVORK5CYII=",
  "base64",
);
const SLUG = "spring-launch";
const TITLE = "Spring launch notes";
const ALT = "Team at the spring launch";

// Template scaffolding, the Astro dev server, and its first route builds are slow here.
test.describe.configure({ timeout: 900_000 });

function git(cwd: string, args: string[]): string {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

async function openBlog(page: Page) {
  await page.getByTestId("workspace-tab-content").click();
  await expect(page.getByTestId("content-panel")).toBeVisible();
  await page.getByTestId("content-collection").filter({ hasText: /^blog/ }).click();
}

/** Go to `route` in the preview frame through the address bar. */
async function previewAt(page: Page, route: string) {
  await page.getByTestId("workspace-tab-preview").click();
  const address = page.getByTestId("preview-address");
  await address.fill(route);
  await address.press("Enter");
}

const frame = (page: Page) => page.frameLocator('[data-testid="preview-frame"]');

test("content editor saves a blog article with an image on a change branch and previews it", async ({
  page,
  workbench,
  templateProject,
}) => {
  const { origin, token } = workbench;
  const { id: projectId, dir: projectDir } = templateProject;

  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("active-change")).toHaveText("main");

  // An existing article opens clean and stays clean through rich → raw → rich.
  await openBlog(page);
  await page.getByTestId("content-entry").filter({ hasText: /^welcome$/ }).click();
  await expect(page.getByTestId("body-rich")).toBeVisible();
  await page.getByTestId("body-mode-raw").click();
  await expect(page.getByTestId("body-raw")).toBeVisible();
  await page.getByTestId("body-mode-rich").click();
  await expect(page.getByTestId("body-rich")).toBeVisible();
  await expect(page.getByTestId("content-save")).toBeDisabled();
  await expect(page.getByTestId("content-rename")).toBeEnabled();
  await expect(page.getByTestId("content-lifecycle-blocked")).toHaveCount(0);

  // Start a change.
  await page.getByTestId("changes-menu").click();
  await page.getByTestId("new-change").click();
  await page.getByLabel("Change title").fill("New post: Spring launch");
  await page.getByRole("button", { name: "Create change" }).click();
  await expect(page.getByTestId("active-change")).toHaveText("Spring launch");
  await expect.poll(() => git(projectDir, ["branch", "--show-current"])).toMatch(/^content\//);
  const branch = git(projectDir, ["branch", "--show-current"]);

  // Create the article (the Content view has no create form; the public API does).
  const created = await api(origin, token, "POST", "/api/content/entry", {
    project: projectId,
    collection: "blog",
    slug: SLUG,
    branch,
    fields: { title: TITLE, description: "What we shipped this spring.", pubDate: "2026-10-11" },
    body: "We shipped the spring launch.\n",
  });
  expect(created.status, await created.clone().text()).toBe(200);

  // It shows up in Content; add an image with the file picker and Save.
  const article = page.getByTestId("content-entry").filter({ hasText: new RegExp(`^${SLUG}$`) });
  await expect(article).toBeVisible();
  await article.click();
  await expect(page.getByTestId("content-panel")).toContainText(`src/content/blog/en/${SLUG}.md`);
  await expect(page.getByTestId("body-rich")).toContainText("We shipped the spring launch.");
  await page.getByTestId("body-rich").locator(".ProseMirror").click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.getByTestId("body-tool-image").click();
  await page.getByTestId("image-file").setInputFiles({
    name: "team.png",
    mimeType: "image/png",
    buffer: TINY_PNG,
  });
  await expect(page.getByTestId("image-preview")).toBeVisible();
  await expect(page.getByTestId("image-insert")).toBeDisabled();
  await page.getByTestId("image-alt").fill(ALT);
  await page.getByTestId("image-insert").click();
  await expect(page.getByTestId("content-save")).toBeEnabled();
  const saved = page.waitForResponse(
    (r) => r.url().includes("/api/content/entry") && r.request().method() === "PUT",
  );
  await page.getByTestId("content-save").click();
  expect((await saved).status()).toBe(200);
  await expect(page.getByTestId("content-save")).toHaveText("Save");
  await expect(page.getByTestId("content-save")).toBeDisabled();
  await expect(page.getByTestId("content-save-error")).toHaveCount(0);

  // On disk: the asset sits in the entry's asset folder and the article links it relatively.
  const assetDir = path.join(projectDir, "src/assets/blog", SLUG);
  const assets = fs.readdirSync(assetDir);
  expect(assets).toHaveLength(1);
  expect(fs.readFileSync(path.join(assetDir, assets[0]!)).equals(TINY_PNG)).toBe(true);
  const source = fs.readFileSync(path.join(projectDir, `src/content/blog/en/${SLUG}.md`), "utf8");
  expect(source).toContain(`![${ALT}](../../../assets/blog/${SLUG}/${assets[0]})`);
  expect(git(projectDir, ["status", "--porcelain"])).toBe("");

  // Preview renders the article route from the change branch, image included.
  await previewAt(page, `/blog/${SLUG}/`);
  await expect(frame(page).getByRole("heading", { level: 1 })).toHaveText(TITLE, {
    timeout: 180_000,
  });
  const image = frame(page).getByAltText(ALT);
  await expect(image).toBeVisible();
  await expect.poll(() => image.evaluate((img: HTMLImageElement) => img.naturalWidth)).toBe(8);

  // The Help index renders the template's docs.
  await previewAt(page, "/help/");
  await expect(frame(page).getByRole("heading", { level: 1 })).toHaveText("Help", {
    timeout: 180_000,
  });
  await expect(frame(page).getByRole("link", { name: "Getting started" })).toBeVisible();

  // On main the article is absent. The Changes picker only lists changes, so the switch to
  // main happens outside the app (as a developer would) and the app is reopened.
  git(projectDir, ["checkout", "main"]);
  await page.reload();
  await expect(page.getByTestId("active-change")).toHaveText("main");
  await openBlog(page);
  await expect(page.getByTestId("content-entry").filter({ hasText: /^welcome$/ })).toBeVisible();
  await expect(article).toHaveCount(0);
  // The home page still renders on main once the dev server has caught up with the checkout.
  await previewAt(page, "/");
  await expect(frame(page).getByRole("link", { name: "Blog" }).first()).toBeVisible({
    timeout: 180_000,
  });
  await previewAt(page, "/blog/");
  await expect(frame(page).getByRole("heading", { level: 1 })).toHaveText("Blog", {
    timeout: 180_000,
  });
  await expect(frame(page).getByRole("link", { name: TITLE })).toHaveCount(0);

  // Leave an unsaved edit open on main, then switch back through the picker: the draft object
  // is kept, and Content and Preview show the change branch again.
  await page.getByTestId("workspace-tab-content").click();
  await page.getByTestId("content-entry").filter({ hasText: /^welcome$/ }).click();
  const titleInput = page.getByTestId("field-title");
  await titleInput.fill("Welcome, edited on main");
  await expect(page.getByTestId("content-save")).toBeEnabled();

  await page.getByTestId("changes-menu").click();
  await page.getByTestId("change-item").filter({ hasText: "Spring launch" }).click();
  await expect(page.getByTestId("active-change")).toHaveText("Spring launch");
  expect(git(projectDir, ["branch", "--show-current"])).toBe(branch);

  await expect(article).toBeVisible();
  await expect(titleInput).toHaveValue("Welcome, edited on main");
  await expect(page.getByTestId("content-save")).toBeEnabled();

  // The kept draft now saves onto the change branch; main keeps its own copy.
  await expect(page.getByTestId("content-panel")).toContainText(branch);
  const resaved = page.waitForResponse(
    (r) => r.url().includes("/api/content/entry") && r.request().method() === "PUT",
  );
  await page.getByTestId("content-save").click();
  expect((await resaved).status()).toBe(200);
  await expect(page.getByTestId("content-save")).toHaveText("Save");
  await expect(page.getByTestId("content-save")).toBeDisabled();
  await expect(page.getByTestId("content-conflict")).toHaveCount(0);
  const welcome = "src/content/blog/en/welcome.md";
  expect(fs.readFileSync(path.join(projectDir, welcome), "utf8")).toContain(
    'title: "Welcome, edited on main"',
  );
  expect(git(projectDir, ["show", `main:${welcome}`])).toContain("title: Welcome to the blog");

  // The preview follows the switch without a manual reload.
  await page.getByTestId("workspace-tab-preview").click();
  await expect(frame(page).getByRole("link", { name: TITLE })).toBeVisible({ timeout: 180_000 });
  await expect(frame(page).getByRole("link", { name: "Welcome, edited on main" })).toBeVisible();

  // The saved article is still on the change branch.
  await page.getByTestId("workspace-tab-content").click();
  await article.click();
  await expect(page.getByTestId("content-panel")).toContainText(`src/content/blog/en/${SLUG}.md`);
  await expect(page.getByTestId("body-rich").getByAltText(ALT)).toBeVisible();
  await expect(page.getByTestId("content-save")).toBeDisabled();
});
