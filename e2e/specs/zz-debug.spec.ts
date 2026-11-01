import fs from "node:fs";
import path from "node:path";

import { expect, test } from "../fixtures/orso";

test("debug preview width after switching tabs", async ({ page, orso }) => {
  fs.writeFileSync(
    path.join(orso.projectDir, "index.html"),
    `<!doctype html><html><body style="margin:0;background:#0a0a0a;color:#fff"><div style="height:3000px">preview</div></body></html>`,
  );

  await page.setViewportSize({ width: 1106, height: 1053 });
  await page.goto(`${orso.origin}/?t=${orso.token}`);
  await page.goto(`${orso.origin}/p/${orso.projectId}/preview`);
  await expect(page.getByTestId("preview-frame")).toBeVisible({ timeout: 120_000 });
  await page.waitForTimeout(1500);

  const measure = () =>
    page.evaluate(() => {
      const rect = (id: string) => {
        const el = document.querySelector(`[data-testid="${id}"]`) as HTMLElement | null;
        if (!el) return null;
        const r = el.getBoundingClientRect();
        return { x: Math.round(r.x), width: Math.round(r.width) };
      };
      const iframe = document.querySelector('[data-testid="preview-frame"]') as HTMLIFrameElement | null;
      return {
        panel: rect("preview-panel"),
        canvas: rect("preview-canvas"),
        frame: rect("preview-frame"),
        width: iframe?.style.width,
        transform: iframe?.style.transform,
      };
    });

  await page.getByTestId("workspace-tab-settings").click();
  await page.waitForTimeout(300);
  await page.getByTestId("workspace-tab-preview").click();

  const samples: unknown[] = [];
  for (let i = 0; i < 40; i++) {
    samples.push(await measure());
    await page.waitForTimeout(16);
  }
  console.log(JSON.stringify(samples, null, 1));
});
