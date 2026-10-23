import { defineConfig, devices } from "@playwright/test";

// Real-browser tests against the real regtest stack (`scripts/e2e.sh` starts and stops it).
export default defineConfig({
  testDir: "./e2e",
  // The tests share one chain and move it forward (mining, close and resolve heights): run them in order.
  fullyParallel: false,
  workers: 1,
  timeout: 240_000,
  expect: { timeout: 45_000 },
  reporter: [["list"], ["html", { open: "never" }]],
  use: {
    baseURL: process.env.E2E_WEB_URL ?? "http://localhost:13000",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    acceptDownloads: true,
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  outputDir: "test-results",
});
