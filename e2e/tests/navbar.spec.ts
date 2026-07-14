import { test, expect } from '@playwright/test';

// Navbar (crates/web/src/views/navbar.rs): a #navbar div wrapping every route, with
// links Playlists (/), Compilers (/compiler), Top 100 (/popular) and Search (/search).
// Every other spec navigates with page.goto; this one drives the actual navbar links.
test.describe('navbar navigation', () => {
  test('drives every navbar link from the home page', async ({ page }) => {
    await page.goto('/');
    const navbar = page.locator('#navbar');
    await expect(navbar.getByRole('link', { name: 'Playlists' })).toBeVisible();

    // -> Compilers: the compiler list view renders (sorted by normalised name, so
    // Captain Moss is first — see compiler.spec.ts).
    await navbar.getByRole('link', { name: 'Compilers' }).click();
    await expect(page).toHaveURL('/compiler');
    await expect(page.locator('table tbody tr td:first-child a').first()).toHaveText('Captain Moss');

    // -> Top 100: the popular tracks view renders with Echoes on top.
    await navbar.getByRole('link', { name: 'Top 100' }).click();
    await expect(page).toHaveURL('/popular');
    await expect(page.locator('a[href="/track/track-echoes-v1"]')).toHaveText('Echoes');

    // -> Search: the search view renders its input.
    await navbar.getByRole('link', { name: 'Search' }).click();
    await expect(page).toHaveURL('/search');
    await expect(page.getByPlaceholder('Search...')).toBeVisible();

    // -> Playlists: back to the home view.
    await navbar.getByRole('link', { name: 'Playlists' }).click();
    await expect(page).toHaveURL('/');
    await expect(page.getByRole('heading', { name: 'The Just Dance Playlist Archives' })).toBeVisible();
  });
});
