import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import type { Page } from "@playwright/test";

import { expect, test } from "../fixtures/orso";

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

/** A row of the Content tree (`blog/` for the collection, `blog/welcome` for an entry). */
const treeRow = (page: Page, treePath: string) =>
  page.getByTestId("content-tree").locator(`[data-item-path="${treePath}"]`);

async function openBlog(page: Page) {
  await page.getByTestId("workspace-tab-content").click();
  await expect(page.getByTestId("content-panel")).toBeVisible();
  const blog = treeRow(page, "blog/");
  await expect(blog).toBeVisible();
  // The tree remembers open folders across reloads; clicking an open one closes it.
  if ((await blog.getAttribute("aria-expanded")) !== "true") await blog.click();
}

/** The entry header's tooltip names the open file and the branch it is bound to. */
const entryBinding = (page: Page, file: string, branch: string) =>
  page.getByTestId("content-panel").getByTitle(`${file} · ${branch}`);

/**
 * Go to `route` in the preview frame through the address bar. A dev-server reload of the page
 * still showing (after a save or checkout) can cancel the navigation, so retry until the address
 * bar, which shows the frame's own path once submitted, reports the route.
 */
async function previewAt(page: Page, route: string) {
  await page.getByTestId("workspace-tab-preview").click();
  const address = page.getByTestId("preview-address");
  await expect(async () => {
    await address.fill(route);
    await address.press("Enter");
    await expect(address).toHaveValue(route, { timeout: 10_000 });
  }).toPass({ timeout: 180_000 });
}

/** Create a change from the open New change dialog. */
async function createChangeInDialog(page: Page, title: string) {
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Change title").fill(title);
  await dialog.getByRole("button", { name: "Create change" }).click();
}

async function saveAndExpect(page: Page, status: number) {
  const response = page.waitForResponse(
    (r) => r.url().includes("/api/content/entry") && r.request().method() === "PUT",
  );
  await page.getByTestId("content-save").click();
  expect((await response).status()).toBe(status);
}

/** The image has loaded real pixels, not a broken link that merely has alt text. */
async function expectLoaded(image: import("@playwright/test").Locator) {
  await expect(image).toBeVisible();
  await expect.poll(() => image.evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth)).toBe(8);
}

const frame = (page: Page) => page.frameLocator('[data-testid="preview-frame"]');

test("content editor saves a blog article with an image on a change branch and previews it", async ({
  page,
  orso,
  templateProject,
}) => {
  const { origin, token } = orso;
  const { id: projectId, dir: projectDir } = templateProject;

  await page.goto(`${origin}/?t=${token}`);
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("active-change")).toHaveText("main");

  // An existing article opens clean in the rich editor.
  await openBlog(page);
  await treeRow(page, "blog/welcome").click();
  await expect(page.getByTestId("body-rich")).toBeVisible();
  await expect(page.getByTestId("content-unsaved")).toHaveCount(0);
  await expect(page.getByTestId("content-rename")).toBeEnabled();
  await expect(page.getByTestId("content-lifecycle-blocked")).toHaveCount(0);

  // Create the article from the Content view. On main that asks for a change first and then
  // resumes the create on the new change branch.
  await treeRow(page, "blog/").click({ button: "right" });
  await page.getByTestId("content-tree-create").click();
  await page.getByTestId("content-create-title").fill(TITLE);
  await expect(page.getByTestId("content-create-slug")).toHaveValue("spring-launch-notes");
  await page.getByTestId("content-create-slug").fill(SLUG);
  await page.getByTestId("content-create-confirm").click();
  await expect(page.getByTestId("content-needs-change")).toBeVisible();
  await page.getByTestId("content-create-change").click();
  await createChangeInDialog(page, "New post: Spring launch");
  await expect(page.getByTestId("active-change")).toHaveText("Spring launch");
  const branch = git(projectDir, ["branch", "--show-current"]);
  expect(branch).toBe("content/spring-launch");
  const article = treeRow(page, `blog/${SLUG}`);
  await expect(article).toBeVisible();
  await expect(entryBinding(page, `src/content/blog/en/${SLUG}.md`, branch)).toBeAttached();
  await expect(page.getByTestId("field-title")).toHaveValue(TITLE);
  // The date picker shows the pre-filled publish date, e.g. "Oct 15, 2026".
  await expect(page.getByTestId("field-pubDate")).toHaveText(/^\s*[A-Z][a-z]{2} \d{1,2}, \d{4}\s*$/);
  expect(fs.existsSync(path.join(projectDir, `src/content/blog/en/${SLUG}.md`))).toBe(true);
  expect(git(projectDir, ["status", "--porcelain", "--untracked-files=no"])).toBe("");

  // Write a line, add an image with the file picker, and Save.
  await page.getByTestId("body-rich").locator(".ProseMirror").click();
  await page.keyboard.type("We shipped the spring launch.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("/");
  await page.getByTestId("slash-item").filter({ hasText: "Image" }).click();
  await page.getByTestId("image-file").setInputFiles({
    name: "team.png",
    mimeType: "image/png",
    buffer: TINY_PNG,
  });
  await expect(page.getByTestId("image-preview")).toBeVisible();
  await expect(page.getByTestId("image-insert")).toBeDisabled();
  await page.getByTestId("image-alt").fill(ALT);
  await page.getByTestId("image-insert").click();
  await saveAndExpect(page, 200);
  await expect(page.getByTestId("content-unsaved")).toHaveCount(0);
  await expect(page.getByTestId("content-save-error")).toHaveCount(0);

  // On disk: the asset sits in the entry's asset folder and the article links it relatively.
  const assetDir = path.join(projectDir, "src/assets/blog", SLUG);
  const assets = fs.readdirSync(assetDir);
  expect(assets).toHaveLength(1);
  expect(fs.readFileSync(path.join(assetDir, assets[0]!)).equals(TINY_PNG)).toBe(true);
  const source = fs.readFileSync(path.join(projectDir, `src/content/blog/en/${SLUG}.md`), "utf8");
  expect(source).toContain("We shipped the spring launch.");
  expect(source).toContain(`![${ALT}](../../../assets/blog/${SLUG}/${assets[0]})`);
  expect(git(projectDir, ["status", "--porcelain"])).toBe("");
  // The saved (relative) image still shows in the editor.
  await expectLoaded(page.getByTestId("body-rich").getByAltText(ALT));

  // Preview renders the article route from the change branch, image included.
  await previewAt(page, `/blog/${SLUG}/`);
  await expect(frame(page).getByRole("heading", { level: 1 })).toHaveText(TITLE, {
    timeout: 180_000,
  });
  await expectLoaded(frame(page).getByAltText(ALT));

  // The Help index renders the template's docs.
  await previewAt(page, "/help/");
  await expect(frame(page).getByRole("heading", { level: 1 })).toHaveText("Help", {
    timeout: 180_000,
  });
  await expect(frame(page).getByRole("link", { name: "Getting started" })).toBeVisible();

  // A draft started on the change branch stays bound to it. Switch away (a second change, made
  // from main): the draft is kept and flagged, and Save there is refused without writing.
  const welcome = "src/content/blog/en/welcome.md";
  const welcomeOnMain = git(projectDir, ["show", `main:${welcome}`]);
  await page.getByTestId("workspace-tab-content").click();
  await treeRow(page, "blog/welcome").click();
  const titleInput = page.getByTestId("field-title");
  // Wait for the entry to load before typing, or the edit lands in the previous entry's form.
  await expect(entryBinding(page, welcome, branch)).toBeAttached();
  await expect(titleInput).toHaveValue("Welcome to the blog");
  await titleInput.fill("Welcome, drafted on the change");
  await expect(page.getByTestId("content-save")).toBeEnabled();

  // The Changes menu hands "New change" to the agent and only lists changes with work of their
  // own, so the second change is branched from main with git (as the agent's guide does), given
  // one commit, and switched to through the menu.
  const otherTip = git(projectDir, [
    "-c",
    "user.name=Agent",
    "-c",
    "user.email=agent@example.com",
    "commit-tree",
    "main^{tree}",
    "-p",
    "main",
    "-m",
    "Start Other idea",
  ]);
  git(projectDir, ["branch", "content/other-idea", otherTip]);
  git(projectDir, ["config", "branch.content/other-idea.description", "Other idea"]);
  await page.getByTestId("changes-menu").click();
  await page.getByTestId("change-item").filter({ hasText: "Other idea" }).click();
  await expect(page.getByTestId("active-change")).toHaveText("Other idea");
  const other = git(projectDir, ["branch", "--show-current"]);
  expect(other).toBe("content/other-idea");
  await expect(article).toHaveCount(0);
  await expect(titleInput).toHaveValue("Welcome, drafted on the change");
  await expect(entryBinding(page, welcome, branch)).toBeAttached();
  await expect(page.getByTestId("content-panel")).toContainText("Content changed on disk");
  await saveAndExpect(page, 409);
  await expect(page.getByTestId("content-conflict")).toBeVisible();
  await expect(titleInput).toHaveValue("Welcome, drafted on the change");
  expect(fs.readFileSync(path.join(projectDir, welcome), "utf8")).toBe(welcomeOnMain + "\n");
  expect(git(projectDir, ["show", `${branch}:${welcome}`])).toBe(welcomeOnMain);
  expect(git(projectDir, ["status", "--porcelain", "--untracked-files=no"])).toBe("");

  // Back on its own branch the flag clears and the same draft saves there; main keeps its copy.
  await page.getByTestId("changes-menu").click();
  await page.getByTestId("change-item").filter({ hasText: "Spring launch" }).click();
  await expect(page.getByTestId("active-change")).toHaveText("Spring launch");
  expect(git(projectDir, ["branch", "--show-current"])).toBe(branch);
  await expect(article).toBeVisible();
  await expect(titleInput).toHaveValue("Welcome, drafted on the change");
  await expect(page.getByTestId("content-panel")).not.toContainText("Content changed on disk");
  await saveAndExpect(page, 200);
  await expect(page.getByTestId("content-unsaved")).toHaveCount(0);
  await expect(page.getByTestId("content-conflict")).toHaveCount(0);
  expect(fs.readFileSync(path.join(projectDir, welcome), "utf8")).toContain(
    'title: "Welcome, drafted on the change"',
  );
  expect(git(projectDir, ["show", `main:${welcome}`])).toBe(welcomeOnMain);
  expect(git(projectDir, ["show", `${other}:${welcome}`])).toBe(welcomeOnMain);

  // On main the article is absent. The Changes picker only lists changes, so the switch to
  // main happens outside the app (as a developer would) and the app is reopened.
  git(projectDir, ["checkout", "main"]);
  await page.reload();
  await expect(page.getByTestId("active-change")).toHaveText("main");
  await openBlog(page);
  await expect(treeRow(page, "blog/welcome")).toBeVisible();
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
  await expect(frame(page).getByRole("link", { name: "Welcome to the blog" })).toBeVisible();

  // Switch back through the picker: Content and Preview follow, and the article is still there.
  await page.getByTestId("changes-menu").click();
  await page.getByTestId("change-item").filter({ hasText: "Spring launch" }).click();
  await expect(page.getByTestId("active-change")).toHaveText("Spring launch");
  await expect(frame(page).getByRole("link", { name: TITLE })).toBeVisible({ timeout: 180_000 });
  await expect(frame(page).getByRole("link", { name: "Welcome, drafted on the change" })).toBeVisible();
  await page.getByTestId("workspace-tab-content").click();
  await expect(article).toBeVisible();
  await article.click();
  await expect(entryBinding(page, `src/content/blog/en/${SLUG}.md`, branch)).toBeAttached();
  await expectLoaded(page.getByTestId("body-rich").getByAltText(ALT));
  await expect(page.getByTestId("content-unsaved")).toHaveCount(0);
});
