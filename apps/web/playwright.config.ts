import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: [[process.env.CI ? "dot" : "list"]],
  use: {
    baseURL: process.env.BASE_URL || "http://127.0.0.1:3000",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: process.env.BASE_URL
    ? undefined
    : {
        command:
          "NODE_OPTIONS='--max-old-space-size=8192' VITE_APP_URL='http://127.0.0.1:3000' VITE_API_URL='http://127.0.0.1:3001' pnpm exec dotenvx run --ignore MISSING_ENV_FILE -f ../../.env.supabase -f .env -- pnpm exec vite dev --host 127.0.0.1 --port 3000 --strictPort",
        url: "http://127.0.0.1:3000",
        reuseExistingServer: !process.env.CI,
      },
});
