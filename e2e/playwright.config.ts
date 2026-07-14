import { defineConfig, devices } from '@playwright/test';
import { resolveDbConfig } from './db-config';

/**
 * Playwright configuration for the Just Dance Playlist Archives e2e suite.
 *
 * Startup ordering (verified against Playwright 1.61: plugin setup tasks, which start
 * the webServer, run BEFORE globalSetup):
 *   1. webServer starts the pre-built Dioxus fullstack binary via
 *      scripts/start-server.sh, possibly against an empty/unseeded database. That is
 *      safe: the server reads MongoDB per request (no cached state), and the readiness
 *      probe below only needs the home route to respond, which it does regardless of
 *      seeded data.
 *   2. global-setup.ts drops and re-seeds the database.
 *   3. Tests run, read-only against the seeded data, so they run fully parallel.
 *
 * Database safety: the connection is resolved by db-config.ts, which refuses any
 * database name that does not start with "playlist_e2e" (the suite drops its database,
 * so ambient production DB_* variables must never leak in). The resolved values are
 * passed to the webServer explicitly below — Playwright merges `webServer.env` over
 * `process.env`, so the server under test always uses the same (guarded) database as
 * the seeding step. Override deliberately with E2E_DB_CONNECTION_STRING / E2E_DB_NAME.
 */
const dbConfig = resolveDbConfig();

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  globalSetup: './global-setup',
  timeout: 60_000,
  expect: {
    // The search view debounces input by 2s before querying, so give web-first
    // assertions enough headroom.
    timeout: 10_000,
  },
  use: {
    baseURL: 'http://127.0.0.1:8811',
    trace: 'on-first-retry',
    // The app renders dates in the local timezone; pin the browser to UTC to match
    // TZ=UTC passed to the server below.
    timezoneId: 'UTC',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'bash scripts/start-server.sh',
    url: 'http://127.0.0.1:8811',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
    // Explicit environment for the server under test (merged over process.env), so
    // ambient DB_*/IP/PORT/TZ values can never redirect it. TZ pins rendered dates to
    // UTC to match `timezoneId` above.
    env: {
      DB_CONNECTION_STRING: dbConfig.uri,
      DB_NAME: dbConfig.dbName,
      IP: '127.0.0.1',
      PORT: '8811',
      TZ: 'UTC',
    },
  },
});
