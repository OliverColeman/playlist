import { test, expect } from '@playwright/test';

// Compiler pages (crates/web/src/views/compiler.rs): /compiler lists all compilers
// sorted by normalised name with their playlists (date descending); /compiler/:id
// shows a single compiler's playlists.
test.describe('compiler pages', () => {
  test('/compiler lists all compilers with their playlists', async ({ page }) => {
    await page.goto('/compiler');

    const rows = page.locator('table tbody tr');
    await expect(rows).toHaveCount(2);

    // Sorted by name_normalised ascending: "captain moss" < "dj pebbles".
    await expect(page.locator('table tbody tr td:first-child a')).toHaveText([
      'Captain Moss',
      'DJ Pebbles',
    ]);

    // Captain Moss compiled Winter Warmer + Autumn Amble (date descending).
    await expect(rows.nth(0).locator('td:nth-child(2) a')).toHaveText([
      'Winter Warmer',
      'Autumn Amble',
    ]);
    // DJ Pebbles compiled Winter Warmer + Summer Solstice Session.
    await expect(rows.nth(1).locator('td:nth-child(2) a')).toHaveText([
      'Winter Warmer',
      'Summer Solstice Session',
    ]);
  });

  test("/compiler/:id shows that compiler's playlists", async ({ page }) => {
    await page.goto('/compiler/compiler-pebbles');

    await expect(page.getByRole('heading', { name: 'Compiler: DJ Pebbles' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Playlists:' })).toBeVisible();

    const rows = page.locator('table.playlist-list tbody tr');
    await expect(rows).toHaveCount(2);
    await expect(page.locator('table.playlist-list tbody tr td:first-child a')).toHaveText([
      'Winter Warmer',
      'Summer Solstice Session',
    ]);
  });

  test('navigating from the compiler list to a compiler', async ({ page }) => {
    await page.goto('/compiler');

    await page.locator('table tbody tr td:first-child a', { hasText: 'Captain Moss' }).click();

    await expect(page).toHaveURL('/compiler/compiler-moss');
    await expect(page.getByRole('heading', { name: 'Compiler: Captain Moss' })).toBeVisible();
    await expect(page.locator('table.playlist-list tbody tr td:first-child a')).toHaveText([
      'Winter Warmer',
      'Autumn Amble',
    ]);
  });
});
