import { test, expect } from '@playwright/test';
import { formatDuration, tracks } from '../fixtures/data';

// Playlist page (crates/web/src/views/playlist.rs): header with name/date/length/
// compilers, then a TrackListComp table listing the playlist's tracks in track_ids
// order. Uses playlist-summer: tracks [Echoes, Golden Sunrise, Midnight Motor,
// Ripple Effect, Paper Lanterns], duration 1012s, date 2024-01-10, compiled by
// DJ Pebbles.
test.describe('playlist page', () => {
  test('shows playlist name, date, length and compiler attribution', async ({ page }) => {
    await page.goto('/playlist/playlist-summer');

    await expect(page.getByRole('heading', { name: 'Playlist: Summer Solstice Session' })).toBeVisible();
    await expect(page.getByText('2024-01-10')).toBeVisible();
    // format_duration(1012) == "16:52", plus the track count.
    await expect(page.getByText('16:52, 5 tracks')).toBeVisible();

    const compilerLink = page.getByRole('link', { name: 'DJ Pebbles' });
    await expect(compilerLink).toBeVisible();
    await expect(compilerLink).toHaveAttribute('href', '/compiler/compiler-pebbles');
  });

  test('lists the tracks in playlist order with artist links and durations', async ({ page }) => {
    await page.goto('/playlist/playlist-summer');

    const rows = page.locator('table.table-fixed tbody tr');
    await expect(rows).toHaveCount(5);

    // Title column follows the playlist's track_ids order.
    await expect(page.locator('table.table-fixed tbody tr td:first-child a')).toHaveText([
      'Echoes',
      'Golden Sunrise',
      'Midnight Motor',
      'Ripple Effect',
      'Paper Lanterns',
    ]);

    // Length column: exact format_duration strings for the fixture durations
    // (215, 187, 240, 198, 172 seconds).
    const expectedDurations = ['track-echoes-v1', 'track-sunrise', 'track-midnight', 'track-ripple', 'track-lanterns']
      .map((id) => formatDuration(tracks.find((t) => t._id === id)!.duration));
    expect(expectedDurations).toEqual(['03:35', '03:07', '04:00', '03:18', '02:52']);
    await expect(page.locator('table.table-fixed tbody tr td:nth-child(4)')).toHaveText(expectedDurations);

    // Artist links.
    await expect(rows.nth(0).getByRole('link', { name: 'Aurora Skye' })).toHaveAttribute(
      'href',
      '/artist/artist-aurora',
    );
    // Paper Lanterns has two artists.
    await expect(rows.nth(4).getByRole('link', { name: 'Vesper Lane' })).toHaveAttribute(
      'href',
      '/artist/artist-vesper',
    );
    await expect(rows.nth(4).getByRole('link', { name: 'Marlow Finch' })).toHaveAttribute(
      'href',
      '/artist/artist-marlow',
    );

    // Album column.
    await expect(rows.nth(0).locator('td:nth-child(3)')).toHaveText('First Light');
    await expect(rows.nth(4).locator('td:nth-child(3)')).toHaveText(''); // no album

    // "List count / Most recent" column for Echoes: the linked pair (v1+v2) appears in
    // all 3 playlists; the most recent is Winter Warmer.
    await expect(rows.nth(0).locator('td:nth-child(5)')).toContainText('3 /');
    await expect(rows.nth(0).getByRole('link', { name: 'Winter Warmer' })).toBeVisible();
  });

  test('clicking a track navigates to the track page', async ({ page }) => {
    await page.goto('/playlist/playlist-summer');

    const trackLink = page.getByRole('link', { name: 'Midnight Motor' });
    await expect(trackLink).toHaveAttribute('href', '/track/track-midnight');
    await trackLink.click();

    await expect(page).toHaveURL('/track/track-midnight');
    await expect(page.getByRole('heading', { name: 'Track: Midnight Motor' })).toBeVisible();
  });
});
