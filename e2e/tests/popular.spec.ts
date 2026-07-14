import { test, expect } from '@playwright/test';
import { POPULAR_TOP3_TRACK_NAMES, POPULAR_TOTAL_TRACKS } from '../fixtures/data';

// Popular page (crates/web/src/views/popular.rs) renders the tracks returned by
// /api/tracks/popular in order. Expected (see the popularity design notes in
// fixtures/data.ts):
//   1. Echoes         (merged linked-track count 4 = v1's 3 + v2's 1)
//   2. Midnight Motor (3)
//   3. Ripple Effect  (2)
// followed by the four remaining count-1 tracks in deterministic track-id order
// (the API sorts by count descending, ties broken by track id).
test.describe('popular tracks page', () => {
  test('shows the expected top tracks in order', async ({ page }) => {
    await page.goto('/popular');

    const rows = page.locator('table.table-fixed tbody tr');
    // 8 seeded tracks minus the merged-away linked version (Echoes (Live)).
    await expect(rows).toHaveCount(POPULAR_TOTAL_TRACKS);

    const titleLinks = page.locator('table.table-fixed tbody tr td:first-child a');
    for (let rank = 0; rank < POPULAR_TOP3_TRACK_NAMES.length; rank++) {
      await expect(titleLinks.nth(rank)).toHaveText(POPULAR_TOP3_TRACK_NAMES[rank]);
    }
    await expect(titleLinks.nth(0)).toHaveAttribute('href', '/track/track-echoes-v1');
    await expect(titleLinks.nth(1)).toHaveAttribute('href', '/track/track-midnight');
    await expect(titleLinks.nth(2)).toHaveAttribute('href', '/track/track-ripple');

    // The merged-away version must not be listed.
    await expect(page.locator('a[href="/track/track-echoes-v2"]')).toHaveCount(0);
  });

  test('shows playlist counts with linked versions merged', async ({ page }) => {
    await page.goto('/popular');

    const rows = page.locator('table.table-fixed tbody tr');
    await expect(rows).toHaveCount(POPULAR_TOTAL_TRACKS);

    // "List count / Most recent": the Echoes pair appears in all 3 playlists, most
    // recently Winter Warmer.
    await expect(rows.nth(0).locator('td:nth-child(5)')).toContainText('3 /');
    await expect(rows.nth(0).getByRole('link', { name: 'Winter Warmer' })).toHaveAttribute(
      'href',
      '/playlist/playlist-winter',
    );
    // Ripple Effect: 2 playlists, most recent Winter Warmer.
    await expect(rows.nth(2).locator('td:nth-child(5)')).toContainText('2 /');
  });
});
