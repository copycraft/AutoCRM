import { defineConfig, devices } from '@playwright/test';

// Browser smoke run (V0.3). Unlike the vitest route tests this drives the real
// stack — Next.js dev server, Rust API, Postgres, MinIO — because the defect
// class it exists to catch (source text rendered as body text, both branches of
// a ternary on screen at once) only shows up in a rendered page.
//
//   docker compose up -d postgres minio minio-init
//   cd backend && cargo run -- migrate && cargo run
//   cd frontend && E2E_EMAIL=... E2E_PASSWORD=... npm run e2e
//
// Screenshots land in frontend/e2e-artifacts/ and are the report.
const baseURL = process.env.E2E_BASE_URL ?? 'http://localhost:3000';

export default defineConfig({
  testDir: './e2e',
  outputDir: './e2e-artifacts/runs',
  timeout: 60_000,
  expect: { timeout: 15_000 },
  fullyParallel: false,
  workers: 1,
  reporter: [['list']],
  use: {
    baseURL,
    locale: 'hu-HU',
    timezoneId: 'Europe/Budapest',
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: process.env.E2E_NO_SERVER
    ? undefined
    : {
        command: 'npm run dev',
        url: baseURL,
        reuseExistingServer: true,
        timeout: 120_000,
      },
});
