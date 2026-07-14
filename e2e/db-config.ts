/**
 * Resolves the MongoDB connection used by BOTH global-setup.ts (which DROPS and
 * re-seeds the database) and the webServer entry in playwright.config.ts (so the
 * server under test and the seeding step always agree).
 *
 * Safety: global-setup drops the whole database, so this must never resolve to a real
 * deployment. Ambient DB_CONNECTION_STRING / DB_NAME (commonly set in developer shells
 * for the app itself) are only honoured if the database name keeps the reserved
 * "playlist_e2e" prefix — anything else aborts the run before anything is dropped.
 * Use E2E_DB_CONNECTION_STRING / E2E_DB_NAME to override deliberately (the prefix rule
 * still applies to the name).
 */

export const E2E_DB_NAME_PREFIX = 'playlist_e2e';

export function resolveDbConfig(): { uri: string; dbName: string } {
  const uri =
    process.env.E2E_DB_CONNECTION_STRING ??
    process.env.DB_CONNECTION_STRING ??
    'mongodb://localhost:27017';
  const dbName = process.env.E2E_DB_NAME ?? process.env.DB_NAME ?? E2E_DB_NAME_PREFIX;

  if (!dbName.startsWith(E2E_DB_NAME_PREFIX)) {
    throw new Error(
      `Refusing to run the e2e suite against database "${dbName}": the suite DROPS its ` +
        `database on startup, and only names starting with "${E2E_DB_NAME_PREFIX}" are ` +
        `allowed. Unset DB_NAME/E2E_DB_NAME, or set E2E_DB_NAME to a ` +
        `"${E2E_DB_NAME_PREFIX}"-prefixed name, and re-run.`,
    );
  }

  return { uri, dbName };
}
