import { test as base } from "@playwright/test";

import type { WorkbenchEnv } from "./daemon";
import { startWorkbench } from "./daemon";

export const test = base.extend<{ workbench: WorkbenchEnv }>({
  workbench: async ({}, use) => {
    const env = await startWorkbench();
    await use(env);
    await env.stop();
  },
});

export { expect } from "@playwright/test";
