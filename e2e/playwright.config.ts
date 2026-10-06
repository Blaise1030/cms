import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "specs",
  timeout: 180_000,
  expect: { timeout: 60_000 },
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  use: {
    headless: true,
    trace: "on-first-retry",
  },
});
