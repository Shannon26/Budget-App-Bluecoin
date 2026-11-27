import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  timeout: 60_000,
  // All specs share one disposable SQLite file (see scripts/e2e.sh). Running
  // files serially keeps one file's seeded accounts from perturbing another's
  // cross-file totals and report snapshots.
  fullyParallel: false,
  workers: 1,
  reporter: "list",
  use: {
    baseURL: "http://127.0.0.1:5173",
    headless: true,
    launchOptions: {
      executablePath: process.env.CHROME_PATH || "/usr/bin/google-chrome",
    },
  },
});
