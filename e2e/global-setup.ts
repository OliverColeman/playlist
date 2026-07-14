import { MongoClient } from 'mongodb';
import { resolveDbConfig } from './db-config';
import { collectionsToSeed } from './fixtures/data';

/**
 * Seeds the e2e database. Runs once per test-suite invocation, AFTER Playwright has
 * started the webServer (plugin setup tasks run before globalSetup — see the ordering
 * note in playwright.config.ts) but before any test runs.
 *
 * Idempotent: the whole database is dropped and re-created from the fixtures in
 * ./fixtures/data.ts. All tests are read-only against this data, so tests can run
 * fully parallel and the suite can be re-run without cleanup.
 *
 * Safety: resolveDbConfig() throws before anything is dropped unless the database
 * name starts with "playlist_e2e" (see db-config.ts).
 */
export default async function globalSetup(): Promise<void> {
  const { uri, dbName } = resolveDbConfig();

  const client = new MongoClient(uri);
  try {
    await client.connect();
    const db = client.db(dbName);
    await db.dropDatabase();
    for (const [collectionName, docs] of Object.entries(collectionsToSeed)) {
      await db.collection(collectionName).insertMany(docs as never[]);
    }
    console.log(`Seeded ${dbName} at ${uri} with fixture data.`);
  } finally {
    await client.close();
  }
}
