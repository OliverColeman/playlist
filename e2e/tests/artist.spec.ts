import { test, expect } from '@playwright/test';
import { ORPHAN_TRACK_ID } from '../fixtures/data';

// Artist page (crates/web/src/views/artist.rs): lists the artist's tracks that appear
// in at least one playlist, sorted by track name.
test.describe('artist page', () => {
  test("lists only the artist's playlisted tracks, sorted by name", async ({ page }) => {
    await page.goto('/artist/artist-aurora');

    await expect(page.getByRole('heading', { name: 'Artist: Aurora Skye' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Tracks:' })).toBeVisible();

    // Aurora Skye has four seeded tracks, but "Unreleased Demo" (the orphan) is in no
    // playlist, so the view's >=1-playlist filter must exclude it: exactly the three
    // playlisted tracks appear, sorted by name.
    const rows = page.locator('table.table-fixed tbody tr');
    await expect(rows).toHaveCount(3);
    await expect(page.locator('table.table-fixed tbody tr td:first-child a')).toHaveText([
      'Echoes',
      'Echoes (Live)',
      'Golden Sunrise',
    ]);
    await expect(page.locator('a[href="/track/track-echoes-v1"]')).toHaveText('Echoes');
    await expect(page.locator('a[href="/track/track-echoes-v2"]')).toHaveText('Echoes (Live)');
    await expect(page.locator('a[href="/track/track-sunrise"]')).toHaveText('Golden Sunrise');

    // The orphan track must not be rendered anywhere on the page.
    await expect(page.locator(`a[href="/track/${ORPHAN_TRACK_ID}"]`)).toHaveCount(0);
    await expect(page.getByText('Unreleased Demo')).toHaveCount(0);
  });

  test('an artist with a single playlisted track', async ({ page }) => {
    await page.goto('/artist/artist-vesper');

    await expect(page.getByRole('heading', { name: 'Artist: Vesper Lane' })).toBeVisible();

    const rows = page.locator('table.table-fixed tbody tr');
    await expect(rows).toHaveCount(2);
    await expect(page.locator('table.table-fixed tbody tr td:first-child a')).toHaveText([
      'Paper Lanterns',
      'Quiet Hours',
    ]);

    // Paper Lanterns is shared with Marlow Finch; both artists are linked on the row.
    await expect(rows.nth(0).getByRole('link', { name: 'Marlow Finch' })).toHaveAttribute(
      'href',
      '/artist/artist-marlow',
    );
  });
});
