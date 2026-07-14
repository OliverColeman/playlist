import { test, expect } from '@playwright/test';
import {
  ORPHAN_TRACK_ID,
  POPULAR_TOP3_TRACK_IDS,
  POPULAR_TOTAL_TRACKS,
  compilers,
  playlists,
} from '../fixtures/data';

// Direct assertions against the server functions in crates/web/src/api.rs.
test.describe('HTTP API', () => {
  test('GET /api/playlists returns exactly the seeded playlists', async ({ request }) => {
    const res = await request.get('/api/playlists');
    expect(res.status()).toBe(200);

    const body = await res.json();
    expect(body.map((p: { _id: string }) => p._id).sort()).toEqual(
      playlists.map((p) => p._id).sort(),
    );

    const summer = body.find((p: { _id: string }) => p._id === 'playlist-summer');
    expect(summer.name).toBe('Summer Solstice Session');
    expect(summer.duration).toBe(1012);
    expect(summer.date).toBe(playlists.find((p) => p._id === 'playlist-summer')!.date);
    expect(summer.compiler_ids).toEqual(['compiler-pebbles']);
  });

  test('GET /api/compilers returns exactly the seeded compilers', async ({ request }) => {
    const res = await request.get('/api/compilers');
    expect(res.status()).toBe(200);

    const body = await res.json();
    expect(body.map((c: { _id: string }) => c._id).sort()).toEqual(
      compilers.map((c) => c._id).sort(),
    );
  });

  test('GET /api/playlists/{id} returns the playlist with tracks keyed by id', async ({ request }) => {
    const res = await request.get('/api/playlists/playlist-summer');
    expect(res.status()).toBe(200);

    const body = await res.json();
    expect(body.playlist._id).toBe('playlist-summer');

    const expectedTrackIds = playlists.find((p) => p._id === 'playlist-summer')!.track_ids;
    expect(Object.keys(body.tracks_by_id).sort()).toEqual([...expectedTrackIds].sort());
    // tracks_by_id must be keyed by each track's own id.
    for (const [key, track] of Object.entries<{ _id: string }>(body.tracks_by_id)) {
      expect(track._id).toBe(key);
    }
    expect(body.tracks_by_id['track-echoes-v1'].name).toBe('Echoes');
    expect(body.tracks_by_id['track-echoes-v1'].artist_ids).toEqual(['artist-aurora']);
    expect(body.artists_by_id['artist-aurora'].name).toBe('Aurora Skye');
    expect(body.albums_by_id['album-first-light'].name).toBe('First Light');
  });

  test('GET /api/tracks/popular returns the deterministic ordering', async ({ request }) => {
    const res = await request.get('/api/tracks/popular');
    expect(res.status()).toBe(200);

    const body = await res.json();
    // The full ordering is deterministic: count descending, ties broken by track id
    // (see the popularity design in fixtures/data.ts). The head has strictly
    // decreasing counts 4, 3, 2; the count-1 tail follows in id order.
    expect(body.sorted_track_ids.slice(0, 3)).toEqual(POPULAR_TOP3_TRACK_IDS);
    expect(body.sorted_track_ids).toEqual([
      ...POPULAR_TOP3_TRACK_IDS,
      'track-lanterns',
      'track-quiet',
      'track-sunrise',
      'track-undertow',
    ]);
    expect(body.sorted_track_ids).toHaveLength(POPULAR_TOTAL_TRACKS);
    // Within-playlist dedup: track-quiet appears 3 times inside playlist-autumn's
    // track_ids but must count once (unique_track_ids in load_popular_tracks).
    // Without dedup its count would be 3, tying track-midnight and evicting
    // track-ripple from the top 3 — so the head assertion above would break, and
    // track-quiet must sit in the count-1 tail, never the head.
    expect(body.sorted_track_ids.slice(0, 3)).not.toContain('track-quiet');
    expect(body.sorted_track_ids).toContain('track-quiet');
    // The orphan track is in no playlist, so it never appears at all.
    expect(body.sorted_track_ids).not.toContain(ORPHAN_TRACK_ID);
    // The less popular linked version is merged into track-echoes-v1.
    expect(body.sorted_track_ids).not.toContain('track-echoes-v2');
    // Its linked-track group is reported alongside.
    expect(body.linked_tracks[0].sort()).toEqual(['track-echoes-v1', 'track-echoes-v2']);
  });

  test('GET /api/search finds the artist by its distinctive name', async ({ request }) => {
    const res = await request.get('/api/search?search_terms=Aurora%20Skye');
    expect(res.status()).toBe(200);

    const body = await res.json();
    // Exactly this artist — an over-returning search must fail here.
    expect(body.artists.map((a: { _id: string }) => a._id)).toEqual(['artist-aurora']);
    // All four Aurora Skye tracks match (artist names are part of track search
    // terms). This deliberately includes the orphan track: search does not filter by
    // playlist membership, unlike the artist page (see artist.spec.ts).
    expect([...body.tracks.sorted_track_ids].sort()).toEqual([
      'track-echoes-v1',
      'track-echoes-v2',
      ORPHAN_TRACK_ID,
      'track-sunrise',
    ]);
    expect(body.compiler_ids).toEqual([]);
    expect(body.playlist_ids).toEqual([]);
  });

  test('GET /api/search finds a playlist by its name', async ({ request }) => {
    const res = await request.get('/api/search?search_terms=Summer%20Solstice%20Session');
    expect(res.status()).toBe(200);

    const body = await res.json();
    expect(body.playlist_ids).toEqual(['playlist-summer']);
    // Nothing else scores above the cutoff for this query.
    expect(body.artists).toEqual([]);
    expect(body.tracks.sorted_track_ids).toEqual([]);
    expect(body.compiler_ids).toEqual([]);
  });

  test('GET /api/search finds a compiler and its playlists by compiler name', async ({ request }) => {
    const res = await request.get('/api/search?search_terms=DJ%20Pebbles');
    expect(res.status()).toBe(200);

    const body = await res.json();
    expect(body.compiler_ids).toEqual(['compiler-pebbles']);
    // Playlists index their compilers' names (build_search_fields — see
    // fixtures/data.ts), so both DJ Pebbles playlists surface too. Their relative
    // order is not deterministic (equal integer scores), so compare sorted.
    expect([...body.playlist_ids].sort()).toEqual(['playlist-summer', 'playlist-winter']);
    expect(body.artists).toEqual([]);
    expect(body.tracks.sorted_track_ids).toEqual([]);
  });

  test('GET /api/search drops candidates that score at or below the cutoff', async ({ request }) => {
    // Unlike the gibberish query in search.spec.ts (which yields zero candidates —
    // no shared n-grams at all), "ora" DOES reach the scoring stage: it shares the
    // n-grams "or"/"ra"/"ora" with "aurora" (artist + track search fields) and "or"
    // with "motor", so the DB $or query returns candidates (observed: 5 tracks and
    // 1 artist). But the best damerau-levenshtein similarity ("ora" vs "aurora" is
    // 0.5) puts every candidate's weighted score at or below the >50 cutoff in
    // search_music_items, so all result sets must come back empty.
    const res = await request.get('/api/search?search_terms=ora');
    expect(res.status()).toBe(200);

    const body = await res.json();
    expect(body.artists).toEqual([]);
    expect(body.tracks.sorted_track_ids).toEqual([]);
    expect(body.compiler_ids).toEqual([]);
    expect(body.playlist_ids).toEqual([]);
  });

  test('GET /api/playlists/{id} for an unknown id returns a 404 error payload', async ({ request }) => {
    // Server functions surface "not found" as a 404 with a JSON error payload.
    const res = await request.get('/api/playlists/does-not-exist');
    expect(res.status()).toBe(404);

    const body = await res.json();
    expect(body.message).toContain('Playlist not found: does-not-exist');
  });

  test('GET /api/tracks/{id} for an unknown id returns a 404 error payload', async ({ request }) => {
    // Same contract as the playlist endpoint; the message quotes the whole requested
    // id list because the endpoint resolves linked-track groups.
    const res = await request.get('/api/tracks/does-not-exist');
    expect(res.status()).toBe(404);

    const body = await res.json();
    expect(body.message).toContain('Track not found: ["does-not-exist"]');
  });

  test('GET /api/artists/{id} for an unknown id returns a 404 error payload', async ({ request }) => {
    const res = await request.get('/api/artists/does-not-exist');
    expect(res.status()).toBe(404);

    const body = await res.json();
    expect(body.message).toContain('Artist not found: "does-not-exist"');
  });
});
