import { test, expect, Page } from '@playwright/test';

// Loads key pages, waits for their hydrated content, and asserts that no console
// errors or uncaught page errors occurred.
//
// One observed benign error is filtered: the binary under test is a `dx build` dev
// build whose client tries to open the Dioxus hot-reload WebSocket (/_dioxus), which
// the standalone server answers with a 404. Everything else is zero-tolerance.
const HOT_RELOAD_WS_ERROR = /WebSocket connection to 'ws:\/\/[^']*\/_dioxus[^']*' failed/;

type ErrorLog = { consoleErrors: string[]; pageErrors: string[] };

function collectErrors(page: Page): ErrorLog {
  const log: ErrorLog = { consoleErrors: [], pageErrors: [] };
  page.on('console', (message) => {
    if (message.type() === 'error' && !HOT_RELOAD_WS_ERROR.test(message.text())) {
      log.consoleErrors.push(message.text());
    }
  });
  page.on('pageerror', (error) => {
    log.pageErrors.push(String(error));
  });
  return log;
}

test.describe('smoke: no console or page errors', () => {
  test('home page', async ({ page }) => {
    const log = collectErrors(page);
    await page.goto('/');
    // Hydrated content: the playlist rows are only rendered client-side.
    await expect(page.getByRole('link', { name: 'Winter Warmer' })).toBeVisible();
    expect(log.consoleErrors).toEqual([]);
    expect(log.pageErrors).toEqual([]);
  });

  test('playlist page', async ({ page }) => {
    const log = collectErrors(page);
    await page.goto('/playlist/playlist-summer');
    await expect(page.getByRole('heading', { name: 'Playlist: Summer Solstice Session' })).toBeVisible();
    await expect(page.getByRole('link', { name: 'Midnight Motor' })).toBeVisible();
    expect(log.consoleErrors).toEqual([]);
    expect(log.pageErrors).toEqual([]);
  });

  test('popular page', async ({ page }) => {
    const log = collectErrors(page);
    await page.goto('/popular');
    await expect(page.getByRole('link', { name: 'Echoes', exact: true })).toBeVisible();
    expect(log.consoleErrors).toEqual([]);
    expect(log.pageErrors).toEqual([]);
  });

  test('search page', async ({ page }) => {
    const log = collectErrors(page);
    await page.goto('/search');
    await expect(page.getByPlaceholder('Search...')).toBeVisible();
    expect(log.consoleErrors).toEqual([]);
    expect(log.pageErrors).toEqual([]);
  });
});
