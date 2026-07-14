import { test, expect } from '@playwright/test';
import { PLAYLIST_NAMES_BY_DATE_DESC, formatDateUtc, playlists } from '../fixtures/data';

// The Home view (crates/web/src/views/home.rs) renders PlaylistListComp, which sorts
// playlists by date descending (PlaylistListComp -> sorted_by_date(-1)).
test.describe('home page', () => {
  test('shows all playlists sorted by date descending', async ({ page }) => {
    await page.goto('/');

    await expect(page.getByRole('heading', { name: 'The Just Dance Playlist Archives' })).toBeVisible();

    const rows = page.locator('table.playlist-list tbody tr');
    // The table body is a loading placeholder until hydration completes.
    await expect(rows).toHaveCount(playlists.length);

    await expect(page.locator('table.playlist-list tbody tr td:first-child a')).toHaveText(
      PLAYLIST_NAMES_BY_DATE_DESC,
    );

    // Date column, same (descending) order: 2024-09-01, 2024-05-20, 2024-01-10.
    const expectedDates = [...playlists]
      .sort((a, b) => b.date - a.date)
      .map((p) => formatDateUtc(p.date));
    await expect(page.locator('table.playlist-list tbody tr td:nth-child(2)')).toHaveText(expectedDates);
  });

  test('shows compiler links and formatted lengths', async ({ page }) => {
    await page.goto('/');

    const rows = page.locator('table.playlist-list tbody tr');
    await expect(rows).toHaveCount(playlists.length);

    // Winter Warmer (row 0) was compiled by both compilers.
    await expect(rows.nth(0).getByRole('link', { name: 'DJ Pebbles' })).toHaveAttribute(
      'href',
      '/compiler/compiler-pebbles',
    );
    await expect(rows.nth(0).getByRole('link', { name: 'Captain Moss' })).toHaveAttribute(
      'href',
      '/compiler/compiler-moss',
    );

    // Length column: format_duration(duration) + ", N tracks".
    await expect(rows.nth(0).locator('td:nth-child(4)')).toHaveText('14:43, 4 tracks');
    await expect(rows.nth(2).locator('td:nth-child(4)')).toHaveText('16:52, 5 tracks');
  });

  test('playlist name links navigate to the playlist page', async ({ page }) => {
    await page.goto('/');

    const link = page.getByRole('link', { name: 'Summer Solstice Session' });
    await expect(link).toHaveAttribute('href', '/playlist/playlist-summer');
    await link.click();

    await expect(page).toHaveURL('/playlist/playlist-summer');
    await expect(page.getByRole('heading', { name: 'Playlist: Summer Solstice Session' })).toBeVisible();
  });
});
