#!/usr/bin/env node
/**
 * Capture the real Orso app for the landing page previews, in light and dark.
 *
 *   ORSO_LINK='http://orso.localhost:7800/?t=<token>' node e2e/scripts/landing-shots.mjs [--only id,id] [--no-video]
 *
 * ORSO_LINK is the link `orso serve` prints (it carries the daemon token). ORSO_PROJECT picks the
 * project (default `cms`). Shots are read-only: they open views, menus and dialogs, and close them
 * without saving, publishing or running an agent.
 *
 * Output: site/src/assets/app/<id>-<light|dark>.png (and public/app/<id>-<scheme>.webm for video
 * shots). site/src/lib/appShots.ts resolves a pair by id; site/src/components/react/AppShot.tsx
 * serves it through <picture> + prefers-color-scheme so it follows the site's own theme.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "@playwright/test";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const outDir = path.join(root, "site/src/assets/app");
const videoDir = path.join(root, "site/public/app");

const link = process.env.ORSO_LINK;
if (!link) {
  console.error("Set ORSO_LINK to the link printed by `orso serve` (http://…/?t=<token>).");
  process.exit(1);
}
const origin = new URL(link).origin;
const project = process.env.ORSO_PROJECT ?? "cms";
const p = (sub) => `/p/${encodeURIComponent(project)}/${sub}`;

const args = process.argv.slice(2);
const only = args.includes("--only") ? args[args.indexOf("--only") + 1].split(",") : null;
const withVideo = !args.includes("--no-video");

/** Landing aspect ratios: hero/agent stage 16:9, ops carousel 3:2, how-it-works 4:3. */
const VIEWPORT = {
  wide: { width: 1600, height: 900 },
  card: { width: 1350, height: 900 },
  square: { width: 1200, height: 900 },
};

const settle = async (page, ms = 800) => {
  await page.waitForLoadState("networkidle").catch(() => {});
  await page.waitForTimeout(ms);
};

/** Wait for the preview frame to paint the site, not the "starting dev server" state. */
async function previewReady(page) {
  await page.getByTestId("preview-frame").waitFor({ timeout: 120_000 });
  await page
    .frameLocator('[data-testid="preview-frame"]')
    .locator("body")
    .waitFor({ timeout: 120_000 });
  await settle(page, 1500);
}

async function openBlog(page) {
  await page.goto(origin + p("content/blog"));
  await page.getByTestId("content-entry-list").waitFor({ timeout: 60_000 });
  await settle(page);
}

async function openFirstEntry(page) {
  await openBlog(page);
  await page.getByTestId("content-entry").first().click();
  await page.getByTestId("content-fields").waitFor({ timeout: 60_000 });
  await settle(page);
}

async function openAgent(page) {
  await page.goto(origin + p("preview"));
  await previewReady(page);
  const panel = page.getByTestId("chat-panel");
  if (!(await panel.isVisible())) await page.getByTestId("chat-toggle").click();
  await panel.waitFor();
  // Show the most recent real conversation when there is one; otherwise the empty composer.
  const history = page.getByTestId("chat-history-toggle");
  if (await history.isVisible()) {
    await history.click();
    const item = page.getByTestId("chat-history-item").first();
    if (await item.isVisible().catch(() => false)) await item.click();
    else await page.keyboard.press("Escape");
  }
  await settle(page, 1200);
}

/**
 * id: file stem. section: where it is used on the landing page. video: also record a short clip
 * of `play` (after `prepare`).
 */
const SHOTS = [
  // Hero: ProductTabs (16:9 stage)
  { id: "hero-content", section: "hero", viewport: VIEWPORT.wide, prepare: openBlog },
  {
    id: "hero-preview",
    section: "hero",
    viewport: VIEWPORT.wide,
    prepare: async (page) => {
      await page.goto(origin + p("preview"));
      await previewReady(page);
    },
    video: true,
    play: async (page) => {
      // Scroll the previewed site so the clip shows a live page, not a still.
      const frame = page.frameLocator('[data-testid="preview-frame"]');
      for (let i = 0; i < 6; i++) {
        await frame.locator("html").evaluate((el) => el.scrollBy({ top: 320, behavior: "smooth" }));
        await page.waitForTimeout(700);
      }
      await frame.locator("html").evaluate((el) => el.scrollTo({ top: 0, behavior: "smooth" }));
      await page.waitForTimeout(900);
    },
  },
  {
    id: "hero-publish",
    section: "hero",
    viewport: VIEWPORT.wide,
    prepare: async (page) => {
      await page.goto(origin + p("preview"));
      await previewReady(page);
      // The changes menu, opened but never confirmed. (share-preview would push the branch.)
      const menu = page.getByTestId("changes-menu");
      if (await menu.isVisible()) await menu.click();
      await settle(page);
    },
  },
  { id: "hero-agent", section: "hero", viewport: VIEWPORT.wide, prepare: openAgent },
  {
    id: "hero-history",
    section: "hero",
    viewport: VIEWPORT.wide,
    prepare: async (page) => {
      await page.goto(origin + p("history"));
      await page.getByTestId("checkpoint-item").first().waitFor({ timeout: 60_000 });
      await settle(page);
    },
  },

  // Ops carousel (3:2 cards)
  { id: "ops-content", section: "ops", viewport: VIEWPORT.card, prepare: openFirstEntry },
  {
    id: "ops-preview",
    section: "ops",
    viewport: VIEWPORT.card,
    prepare: async (page) => {
      await page.goto(origin + p("preview"));
      await previewReady(page);
      // Switch the canvas to a phone width when the viewport control offers it.
      const size = page.getByTestId("viewport-size");
      if (await size.isVisible()) {
        await size.click();
        const mobile = page.getByRole("menuitemradio", { name: /mobile|phone/i }).or(page.getByRole("option", { name: /mobile|phone/i }));
        if (await mobile.first().isVisible().catch(() => false)) await mobile.first().click();
        else await page.keyboard.press("Escape");
      }
      await previewReady(page);
    },
  },
  {
    id: "ops-publish",
    section: "ops",
    viewport: VIEWPORT.card,
    prepare: async (page) => {
      await page.goto(origin + p("preview"));
      await previewReady(page);
      const menu = page.getByTestId("changes-menu");
      if (await menu.isVisible()) await menu.click();
      await settle(page);
    },
  },
  { id: "ops-agent", section: "ops", viewport: VIEWPORT.card, prepare: openAgent },
  {
    id: "ops-history",
    section: "ops",
    viewport: VIEWPORT.card,
    prepare: async (page) => {
      await page.goto(origin + p("history"));
      await page.getByTestId("checkpoint-item").first().waitFor({ timeout: 60_000 });
      const version = page.getByTestId("checkpoint-item").nth(1);
      if (await version.isVisible().catch(() => false)) await version.click();
      await settle(page, 1200);
    },
  },

  // How it works (4:3 cards)
  {
    id: "how-drafts",
    section: "how",
    viewport: VIEWPORT.square,
    prepare: async (page) => {
      await page.goto(origin + p("history"));
      await page.getByTestId("checkpoint-item").first().waitFor({ timeout: 60_000 });
      await settle(page);
    },
  },
  {
    id: "how-review",
    section: "how",
    viewport: VIEWPORT.square,
    prepare: async (page) => {
      await page.goto(origin + p("preview"));
      await previewReady(page);
      const details = page.getByTestId("page-details-toggle");
      if (await details.isVisible()) await details.click();
      await settle(page, 1200);
    },
  },
  { id: "how-agent", section: "how", viewport: VIEWPORT.square, prepare: openAgent },

  // Agent band (16:9, with a clip)
  {
    id: "agent-band",
    section: "agent",
    viewport: VIEWPORT.wide,
    prepare: openAgent,
    video: true,
    play: async (page) => {
      const viewport = page.getByTestId("chat-viewport");
      for (let i = 0; i < 5; i++) {
        await viewport.evaluate((el) => el.scrollBy({ top: 260, behavior: "smooth" })).catch(() => {});
        await page.waitForTimeout(800);
      }
      await page.waitForTimeout(800);
    },
  },
];

async function newContext(browser, scheme, viewport, video) {
  const context = await browser.newContext({
    viewport,
    deviceScaleFactor: video ? 1 : 2,
    colorScheme: scheme,
    reducedMotion: video ? "no-preference" : "reduce",
    ...(video ? { recordVideo: { dir: path.join(outDir, ".video-tmp"), size: viewport } } : {}),
  });
  await context.addInitScript(() => {
    localStorage.setItem("cms:setup-dismissed", "1");
    localStorage.removeItem("orso.theme"); // follow colorScheme
  });
  // Authenticate with the daemon link on a throwaway tab (the app stores the token), so the
  // shot tab and its video start clean.
  const auth = await context.newPage();
  await auth.goto(link);
  await settle(auth, 500);
  await auth.close();
  const page = await context.newPage();
  return { context, page };
}

async function capture(browser, shot, scheme) {
  const base = path.join(outDir, `${shot.id}-${scheme}`);
  {
    const { context, page } = await newContext(browser, scheme, shot.viewport, false);
    try {
      await shot.prepare(page);
      await page.screenshot({ path: `${base}.png`, animations: "disabled", caret: "hide" });
      console.log(`✓ ${path.relative(root, base)}.png`);
    } finally {
      await context.close();
    }
  }
  if (!(shot.video && withVideo)) return;
  const { context, page } = await newContext(browser, scheme, shot.viewport, true);
  try {
    await shot.prepare(page);
    await shot.play(page);
  } finally {
    await context.close();
  }
  const raw = await page.video().path();
  fs.mkdirSync(videoDir, { recursive: true });
  const dest = path.join(videoDir, `${shot.id}-${scheme}.webm`);
  fs.renameSync(raw, dest);
  console.log(`✓ ${path.relative(root, dest)}`);
}

fs.mkdirSync(outDir, { recursive: true });
const browser = await chromium.launch();
let failed = 0;
try {
  for (const shot of SHOTS.filter((s) => !only || only.includes(s.id))) {
    for (const scheme of ["light", "dark"]) {
      try {
        await capture(browser, shot, scheme);
      } catch (error) {
        failed++;
        console.error(`✗ ${shot.id}-${scheme}: ${error.message.split("\n")[0]}`);
      }
    }
  }
} finally {
  await browser.close();
  fs.rmSync(path.join(outDir, ".video-tmp"), { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
