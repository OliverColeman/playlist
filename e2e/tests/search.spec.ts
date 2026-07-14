import { test, expect, Page, Locator } from '@playwright/test';

// Search page (crates/web/src/views/search.rs): a text input (debounced by 2 seconds)
// that queries /api/search, then renders result tabs (Tracks / Artists / Playlists /
// Compilers) with "Tracks" selected by default. Empty result sets render "Nada.".

/** A result tab in the tab bar above the results. The navbar also has "Playlists" /
 * "Compilers" links, so scope to the tab bar container (the div with class mb-4). */
function resultTab(page: Page, name: string): Locator {
  return page.locator(`div.mb-4 > a:text-is("${name}")`);
}

// Typing requires the wasm oninput handler, so `searchFor` retries the fill until the
// result tab bar appears (the tab bar only renders once a search has completed, which
// proves hydration + search round-trip). No fixed timeouts are used — toPass retries
// on concrete UI signals. The windows are deliberately generous: the input is
// debounced by 2s before the query even fires, so each attempt gets 15s, and the
// outer toPass window is kept very wide for slow CI machines — determinism first,
// speed second (the matching per-test timeout is raised in beforeEach below).
async function searchFor(page: Page, text: string): Promise<void> {
  await page.goto('/search');
  const input = page.getByPlaceholder('Search...');
  await expect(input).toBeVisible();

  await expect(async () => {
    await input.fill(text);
    // "Artists" only exists in the result tab bar (the navbar has no such link).
    await expect(resultTab(page, 'Artists')).toBeVisible({ timeout: 15_000 });
  }).toPass({ timeout: 120_000, intervals: [500, 1_000] });
}

test.describe('search page', () => {
  // Headroom for searchFor's wide retry window (default test timeout is 60s).
  test.beforeEach(async () => {
    test.setTimeout(180_000);
  });

  test('finds an artist by name', async ({ page }) => {
    await searchFor(page, 'Aurora Skye');

    await resultTab(page, 'Artists').click();

    const artistLink = page.locator('a[href="/artist/artist-aurora"]');
    await expect(artistLink).toHaveText('Aurora Skye');

    await artistLink.click();
    await expect(page).toHaveURL('/artist/artist-aurora');
    await expect(page.getByRole('heading', { name: 'Artist: Aurora Skye' })).toBeVisible();
  });

  test('finds a track by exact name', async ({ page }) => {
    await searchFor(page, 'Midnight Motor');

    // The "Tracks" tab is selected by default.
    const trackLink = page.locator('a[href="/track/track-midnight"]');
    await expect(trackLink).toHaveText('Midnight Motor');
  });

  test('finds playlists and compilers by compiler name', async ({ page }) => {
    await searchFor(page, 'DJ Pebbles');

    // Playlists index their compilers' names (see fixtures/data.ts), so both DJ
    // Pebbles playlists match. Their relative order is not deterministic (equal
    // scores), so assert membership + count rather than order.
    await resultTab(page, 'Playlists').click();
    const playlistRows = page.locator('table.playlist-list tbody tr');
    await expect(playlistRows).toHaveCount(2);
    await expect(
      page.locator('table.playlist-list a[href="/playlist/playlist-summer"]'),
    ).toBeVisible();
    await expect(
      page.locator('table.playlist-list a[href="/playlist/playlist-winter"]'),
    ).toBeVisible();

    await resultTab(page, 'Compilers').click();
    const compilerLink = page.locator('a[href="/compiler/compiler-pebbles"]');
    await expect(compilerLink).toHaveText('DJ Pebbles');

    await compilerLink.click();
    await expect(page).toHaveURL('/compiler/compiler-pebbles');
    await expect(page.getByRole('heading', { name: 'Compiler: DJ Pebbles' })).toBeVisible();
  });

  test('shows an empty state for a gibberish query', async ({ page }) => {
    await searchFor(page, 'zzqx');

    // No tracks match: the default "Tracks" tab renders "Nada.".
    await expect(page.getByText('Nada.', { exact: true })).toBeVisible();
    await expect(page.locator('table.table-fixed tbody tr')).toHaveCount(0);
  });
});
