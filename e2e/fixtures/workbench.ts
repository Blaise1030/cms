import path from "node:path";

import { expect, test as base, type Page } from "@playwright/test";

import type { WorkbenchEnv } from "./daemon";
import { api, startWorkbench } from "./daemon";

/** A local-only project scaffolded from the bundled astro-marketing template. */
export type TemplateProject = { id: string; dir: string };

export const test = base.extend<{ workbench: WorkbenchEnv; templateProject: TemplateProject }>({
  // Specs start signed out of GitHub (and may lack gh), which would put the first-run setup
  // check in front of the app. Start as a user who already moved past it; the check itself is
  // covered by the web unit tests.
  context: async ({ context }, use) => {
    await context.addInitScript(() => localStorage.setItem("cms:setup-dismissed", "1"));
    await use(context);
  },
  workbench: async ({}, use) => {
    const env = await startWorkbench();
    await use(env);
    await env.stop();
  },
  // Scaffolding runs `npm install` for the template, which is slow; it gets its own budget.
  templateProject: [
    async ({ workbench }, use) => {
      const res = await api(workbench.origin, workbench.token, "POST", "/api/projects", {
        name: "Marketing site",
      });
      if (!res.ok) throw new Error(`template project create failed: ${await res.text()}`);
      const { id } = (await res.json()) as { id: string };
      await use({ id, dir: path.join(workbench.tmp, "projects", id) });
    },
    { timeout: 600_000 },
  ],
});

/**
 * Pick the e2e `fake` CLI. Exact match is required: the fixture also registers
 * `fake-resolver` and `fake-garbage`, which substring-match `name: "fake"`.
 */
export async function selectFakeProvider(page: Page) {
  await page
    .getByTestId("terminal-provider")
    .getByRole("radio", { name: "fake", exact: true })
    .check();
}

/** Open the header's pull request menu (status, GitHub link, Ready for review). Escape closes it. */
export async function openPrMenu(page: Page) {
  await page.getByTestId("pr-menu").click();
  await expect(page.getByTestId("pr-link")).toBeVisible();
}

export { expect };
