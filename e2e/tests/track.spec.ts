import { test, expect, Page } from '@playwright/test';
import { PLAYLIST_NAMES_BY_DATE_DESC } from '../fixtures/data';

// Track page (crates/web/src/views/track.rs): header with name/artists/album/length,
// an "Appears in playlists:" PlaylistListComp, and an "All versions of this track:"
// TrackListComp fed from the linked_track document. track-echoes-v1 ("Echoes") is
// linked with track-echoes-v2 ("Echoes (Live)").
//
// The header renders each field as `div { h6 { "Label:" } div { value } }`. The
// "All versions of this track" table repeats the album name, the duration and the
// artist links, so header assertions MUST anchor on the h6 label — a bare
// page.getByText(...) would still pass via the table even if the header broke.
function headerValue(page: Page, label: string) {
  return page.locator(`div:has(> h6:text-is("${label}"))`).locator('> div');
}

test.describe('track page', () => {
  test('shows name, artist link, album and length', async ({ page }) => {
    await page.goto('/track/track-echoes-v1');

    await expect(page.getByRole('heading', { name: 'Track: Echoes', exact: true })).toBeVisible();

    // Header artist link, scoped to the "Artist(s):" field (the versions table
    // repeats the artist link).
    const artistLink = headerValue(page, 'Artist(s):').getByRole('link', { name: 'Aurora Skye' });
    await expect(artistLink).toBeVisible();
    await expect(artistLink).toHaveAttribute('href', '/artist/artist-aurora');

    await expect(headerValue(page, 'Album:')).toHaveText('First Light');
    // format_duration(215) == "03:35".
    await expect(headerValue(page, 'Length:')).toHaveText('03:35');
  });

  test('lists the playlists the track (including linked versions) appears in', async ({ page }) => {
    await page.goto('/track/track-echoes-v1');

    await expect(page.getByRole('heading', { name: 'Appears in playlists:' })).toBeVisible();

    // Echoes v1 is in all three playlists (v2 only in Winter Warmer, which is already
    // counted); sorted by date descending.
    const playlistRows = page.locator('table.playlist-list tbody tr');
    await expect(playlistRows).toHaveCount(3);
    await expect(page.locator('table.playlist-list tbody tr td:first-child a')).toHaveText(
      PLAYLIST_NAMES_BY_DATE_DESC,
    );
  });

  test('represents both versions of a linked track', async ({ page }) => {
    await page.goto('/track/track-echoes-v1');

    await expect(page.getByRole('heading', { name: 'All versions of this track:' })).toBeVisible();

    // The versions table contains one row per linked version (row order is not
    // deterministic — the view iterates a HashMap — so assert by href).
    const versionsTable = page.locator('table.table-fixed');
    await expect(versionsTable.locator('tbody tr')).toHaveCount(2);
    await expect(versionsTable.locator('a[href="/track/track-echoes-v1"]')).toHaveText('Echoes');
    await expect(versionsTable.locator('a[href="/track/track-echoes-v2"]')).toHaveText('Echoes (Live)');
  });

  test('the other linked version shows the same versions list', async ({ page }) => {
    await page.goto('/track/track-echoes-v2');

    await expect(page.getByRole('heading', { name: 'Track: Echoes (Live)' })).toBeVisible();

    const versionsTable = page.locator('table.table-fixed');
    await expect(versionsTable.locator('tbody tr')).toHaveCount(2);
    await expect(versionsTable.locator('a[href="/track/track-echoes-v1"]')).toHaveText('Echoes');
    await expect(versionsTable.locator('a[href="/track/track-echoes-v2"]')).toHaveText('Echoes (Live)');
  });

  test('artist link navigates to the artist page', async ({ page }) => {
    await page.goto('/track/track-echoes-v1');

    await headerValue(page, 'Artist(s):').getByRole('link', { name: 'Aurora Skye' }).click();

    await expect(page).toHaveURL('/artist/artist-aurora');
    await expect(page.getByRole('heading', { name: 'Artist: Aurora Skye' })).toBeVisible();
  });

  test('a track without an album shows a placeholder', async ({ page }) => {
    await page.goto('/track/track-quiet');

    await expect(page.getByRole('heading', { name: 'Track: Quiet Hours' })).toBeVisible();
    // Album: "-" (crates/web/src/views/track.rs renders "-" when there is no album),
    // asserted in the header field itself — a '-' anywhere else must not satisfy this.
    await expect(headerValue(page, 'Album:')).toHaveText('-');
  });
});
