/**
 * Seed fixtures for the Playwright e2e suite.
 *
 * The documents built here are inserted into the `playlist_e2e` MongoDB database by
 * `global-setup.ts` and must deserialize into the Rust structs defined in
 * `crates/core/src/models/`. Field names are snake_case and the id field is `_id`.
 *
 * The normalisation / n-gram helpers replicate the pure functions in
 * `crates/core/src/lib.rs` (fixture names are kept ASCII so no deunicode step is
 * needed), and search-field composition mirrors `build_search_fields` in
 * `crates/cli/src/commands/import_playlist.rs` / the migration:
 *   - tracks index their own name plus their artists' names,
 *   - playlists index their own name plus their compilers' names.
 * `search_double_metaphone_codes` is left empty: the search endpoint also matches via
 * the n-gram branch, and scoring (damerau-levenshtein on search_terms) gives exact-name
 * queries a score of 100, above the 50 cutoff.
 */

/** Mirrors `remove_punctuation` + `remove_multiple_spaces_and_trim` + lowercasing in
 * `normalise_name` (crates/core/src/lib.rs). */
export function normaliseName(s: string): string {
  let out = s.toLowerCase();
  out = out.replace(/['"]/g, '');
  out = out.replace(/[\/\\()\[\]{}<>\-_;:,]/g, ' ');
  return out.split(/\s+/).filter(Boolean).join(' ');
}

/** Mirrors `normalise_name_strong` (crates/core/src/lib.rs). */
export function normaliseNameStrong(s: string): string {
  let out = s.toLowerCase().trim();
  if (out.length < 2) {
    return out;
  }
  // Remove anything in brackets, except a bracket at the very start.
  const first = out[0];
  const rest = out.slice(1).replace(/(\([^)]*\)|\[[^\]]*\]|<[^>]*>)/g, '');
  out = first + rest;
  // Remove anything after the first hyphen.
  out = out.replace(/^([^-]+)-.*$/, '$1');
  // Remove anything after "feat" / "feat.".
  out = out.replace(/^([\s\S]*?)(\s+feat\.?\s[\s\S]*)$/, '$1');
  // Remove punctuation.
  out = out.replace(/['"]/g, '');
  out = out.replace(/[\/\\()\[\]{}<>\-_;:,]/g, ' ');
  // Remove "remastered" / "remix" / "radio edit" and similar.
  out = out.replace(/(\s\d{4})?\s(digital(ly)?\s)?remaster(ed)?(\sversion)?(\s\d{4})?/g, '');
  out = out.replace(/(\s+(remix|radio\s+(edit|cut|mix|version))\s*)/g, '');
  return out.split(/\s+/).filter(Boolean).join(' ');
}

/** Mirrors `generate_n_grams` (crates/core/src/lib.rs): all unique 2- and 3-char
 * substrings of a single word, in first-seen order. */
export function nGramsForWord(word: string): string[] {
  const grams: string[] = [];
  for (let n = 2; n <= 3; n++) {
    if (word.length >= n) {
      for (let i = 0; i + n <= word.length; i++) {
        const gram = word.slice(i, i + n);
        if (!grams.includes(gram)) {
          grams.push(gram);
        }
      }
    }
  }
  return grams;
}

/** Mirrors `build_search_fields` (crates/cli): words of the already-normalised search
 * strings become search_terms; n-grams are generated per word. */
function buildSearchFields(normalisedSearchStrings: string[]) {
  const terms = new Set<string>();
  const nGrams = new Set<string>();
  for (const searchString of normalisedSearchStrings) {
    for (const word of searchString.split(' ').filter(Boolean)) {
      terms.add(word);
      for (const gram of nGramsForWord(word)) {
        nGrams.add(gram);
      }
    }
  }
  return {
    search_terms: [...terms],
    search_double_metaphone_codes: [] as string[],
    search_n_grams: [...nGrams],
  };
}

/** Common fields required by every music item struct (see the
 * `define_music_item_struct_with_common_fields!` macro in crates/core). */
function baseItem(id: string, name: string, extraSearchNames: string[] = []) {
  const nameNormalised = normaliseName(name);
  return {
    _id: id,
    name,
    name_normalised: nameNormalised,
    name_normalised_strong: normaliseNameStrong(name),
    ...buildSearchFields([nameNormalised, ...extraSearchNames.map(normaliseName)]),
  };
}

/** Mirrors `format_duration` in crates/web/src/util.rs: "[H:]MM:SS". */
export function formatDuration(seconds: number): string {
  const total = Math.round(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  const pad = (n: number) => String(n).padStart(2, '0');
  if (hours === 0) {
    return `${pad(minutes)}:${pad(secs)}`;
  }
  return `${hours}:${pad(minutes)}:${pad(secs)}`;
}

/** The app renders dates with `format_date` (crates/web/src/util.rs) in the *local*
 * timezone. The test setup pins TZ=UTC for the server (scripts/start-server.sh) and
 * timezoneId "UTC" for the browser (playwright.config.ts), and all fixture dates are at
 * 12:00 UTC, so `toISOString` gives the exact rendered "YYYY-MM-DD" string. */
export function formatDateUtc(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toISOString().slice(0, 10);
}

const utcNoon = (year: number, month: number, day: number): number =>
  Date.UTC(year, month - 1, day, 12, 0, 0) / 1000;

// ---------------------------------------------------------------------------
// Artists
// ---------------------------------------------------------------------------

export const artists = [
  // "Aurora Skye" is the distinctive searchable name used by search.spec.ts.
  { ...baseItem('artist-aurora', 'Aurora Skye') },
  { ...baseItem('artist-marlow', 'Marlow Finch') },
  { ...baseItem('artist-tidal', 'The Tidal Waves') },
  { ...baseItem('artist-vesper', 'Vesper Lane') },
];

// ---------------------------------------------------------------------------
// Albums
// ---------------------------------------------------------------------------

export const albums = [
  { ...baseItem('album-first-light', 'First Light'), artist_ids: ['artist-aurora'] },
  { ...baseItem('album-night-drive', 'Night Drive'), artist_ids: ['artist-marlow'] },
  { ...baseItem('album-undertow', 'Undertow'), artist_ids: ['artist-tidal'] },
];

// ---------------------------------------------------------------------------
// Tracks
// ---------------------------------------------------------------------------
// "Echoes" / "Echoes (Live)" are two versions of the same song: both normalise
// (strongly) to "echoes" and are joined by the linked_track document below.
// "Unreleased Demo" (ORPHAN_TRACK_ID) is deliberately in no playlist — see its
// inline comment.

/** A track (by Aurora Skye) that appears in no playlist; see the comment on its
 * fixture entry below. */
export const ORPHAN_TRACK_ID = 'track-orphan';

export const tracks = [
  {
    ...baseItem('track-echoes-v1', 'Echoes', ['Aurora Skye']),
    artist_ids: ['artist-aurora'],
    album_id: 'album-first-light',
    duration: 215, // 03:35
  },
  {
    ...baseItem('track-echoes-v2', 'Echoes (Live)', ['Aurora Skye']),
    artist_ids: ['artist-aurora'],
    duration: 230, // 03:50
  },
  {
    ...baseItem('track-sunrise', 'Golden Sunrise', ['Aurora Skye']),
    artist_ids: ['artist-aurora'],
    album_id: 'album-first-light',
    duration: 187, // 03:07
  },
  {
    ...baseItem('track-midnight', 'Midnight Motor', ['Marlow Finch']),
    artist_ids: ['artist-marlow'],
    album_id: 'album-night-drive',
    duration: 240, // 04:00
  },
  {
    ...baseItem('track-ripple', 'Ripple Effect', ['The Tidal Waves']),
    artist_ids: ['artist-tidal'],
    album_id: 'album-undertow',
    duration: 198, // 03:18
  },
  {
    ...baseItem('track-undertow', 'Undertow', ['The Tidal Waves']),
    artist_ids: ['artist-tidal'],
    album_id: 'album-undertow',
    duration: 203, // 03:23
  },
  {
    ...baseItem('track-quiet', 'Quiet Hours', ['Vesper Lane']),
    artist_ids: ['artist-vesper'],
    duration: 165, // 02:45
  },
  {
    ...baseItem('track-lanterns', 'Paper Lanterns', ['Vesper Lane', 'Marlow Finch']),
    artist_ids: ['artist-vesper', 'artist-marlow'],
    duration: 172, // 02:52
  },
  {
    // Deliberately referenced by NO playlist ("orphan" track): the artist page
    // (crates/web/src/views/artist.rs) filters an artist's tracks to those appearing
    // in >= 1 playlist, so artist.spec.ts asserts this track is absent from the
    // Aurora Skye page. Having no playlist occurrences it cannot affect
    // /api/tracks/popular or the home page, but it DOES surface in /api/search
    // results for "Aurora Skye" (search does not filter by playlist membership —
    // api.spec.ts includes it deliberately).
    ...baseItem(ORPHAN_TRACK_ID, 'Unreleased Demo', ['Aurora Skye']),
    artist_ids: ['artist-aurora'],
    duration: 149, // 02:29
  },
];

// ---------------------------------------------------------------------------
// Compilers
// ---------------------------------------------------------------------------

export const compilers = [
  { ...baseItem('compiler-pebbles', 'DJ Pebbles') },
  { ...baseItem('compiler-moss', 'Captain Moss') },
];

// ---------------------------------------------------------------------------
// Playlists
// ---------------------------------------------------------------------------
// Popularity design (drives /popular and /api/tracks/popular — see
// load_popular_tracks in crates/web/src/api.rs):
//
//   raw playlist-occurrence counts (each playlist counts a track AT MOST ONCE —
//   load_popular_tracks dedups repeated track ids within a playlist):
//     track-echoes-v1  3  (summer, autumn, winter)
//     track-echoes-v2  1  (winter)                  <- linked with v1
//     track-midnight   3  (summer, autumn, winter)
//     track-ripple     2  (summer, winter)
//     every other playlisted track 1
//       (incl. track-quiet, which appears THREE times within playlist-autumn's
//        track_ids: with dedup it still counts 1; without dedup it would count 3,
//        tying track-midnight and evicting track-ripple from rank 3 — api.spec.ts
//        pins this)
//     track-orphan     -  (in no playlist, so it never appears on /popular)
//
//   The linked pair (v1=3, v2=1) is merged: the endpoint walks tracks in ascending
//   count order, so the most-popular version (v1, visited last) deterministically
//   absorbs the group's counts, giving track-echoes-v1 a merged count of 4.
//
//   The full /api/tracks/popular ordering is deterministic: count descending, ties
//   broken by track id (aggregate_popular_tracks in crates/web/src/api.rs). Head:
//     1. track-echoes-v1  (4, merged 3 + 1)
//     2. track-midnight   (3)
//     3. track-ripple     (2)
//   followed by the count-1 tracks in track-id order.

export const playlists = [
  {
    ...baseItem('playlist-summer', 'Summer Solstice Session', ['DJ Pebbles']),
    compiler_ids: ['compiler-pebbles'],
    track_ids: ['track-echoes-v1', 'track-sunrise', 'track-midnight', 'track-ripple', 'track-lanterns'],
    duration: 1012, // 215+187+240+198+172 -> "16:52"
    user_id: 'user-e2e',
    date: utcNoon(2024, 1, 10), // renders as 2024-01-10
  },
  {
    ...baseItem('playlist-autumn', 'Autumn Amble', ['Captain Moss']),
    compiler_ids: ['compiler-moss'],
    // track-quiet appears three times ON PURPOSE: it exercises the within-playlist
    // dedup in load_popular_tracks (see the popularity design above). Its popularity
    // count must stay 1. Note the playlist page/home page render one row per
    // track_ids entry and "N tracks" is track_ids.len(), so this playlist renders 6
    // rows / "6 tracks" (no spec asserts this playlist's rows or length).
    track_ids: [
      'track-echoes-v1',
      'track-midnight',
      'track-undertow',
      'track-quiet',
      'track-quiet',
      'track-quiet',
    ],
    duration: 1153, // 215+240+203+165*3 -> "19:13"
    user_id: 'user-e2e',
    date: utcNoon(2024, 5, 20), // renders as 2024-05-20
  },
  {
    ...baseItem('playlist-winter', 'Winter Warmer', ['DJ Pebbles', 'Captain Moss']),
    compiler_ids: ['compiler-pebbles', 'compiler-moss'],
    track_ids: ['track-echoes-v1', 'track-echoes-v2', 'track-midnight', 'track-ripple'],
    duration: 883, // 215+230+240+198 -> "14:43"
    user_id: 'user-e2e',
    date: utcNoon(2024, 9, 1), // renders as 2024-09-01
  },
];

// ---------------------------------------------------------------------------
// Linked tracks: "Echoes" and "Echoes (Live)" are the same song.
// ---------------------------------------------------------------------------

export const linkedTracks = [
  {
    _id: 'linked-echoes',
    track_name_normalised_strong: 'echoes',
    track_ids: ['track-echoes-v1', 'track-echoes-v2'],
    artist_ids: ['artist-aurora'],
  },
];

/** Collection name -> documents, as consumed by global-setup.ts. */
export const collectionsToSeed: Record<string, object[]> = {
  artist: artists,
  album: albums,
  track: tracks,
  compiler: compilers,
  playlist: playlists,
  linked_track: linkedTracks,
};

// ---------------------------------------------------------------------------
// Derived expectations shared by the specs.
// ---------------------------------------------------------------------------

/** Home page ordering: playlists sorted by date descending (see PlaylistListComp). */
export const PLAYLIST_NAMES_BY_DATE_DESC = ['Winter Warmer', 'Autumn Amble', 'Summer Solstice Session'];

/** Deterministic head of /api/tracks/popular (see popularity design above). */
export const POPULAR_TOP3_TRACK_IDS = ['track-echoes-v1', 'track-midnight', 'track-ripple'];
export const POPULAR_TOP3_TRACK_NAMES = ['Echoes', 'Midnight Motor', 'Ripple Effect'];

/** Track ids that appear in at least one playlist (repeats within a playlist collapse
 * in the Set; ORPHAN_TRACK_ID is deliberately absent). */
const playlistedTrackIds = new Set(playlists.flatMap((p) => p.track_ids));

/** Total tracks on /popular: the playlisted tracks minus the merged-away linked
 * version (echoes-v2 is folded into v1). The orphan track never contributes. */
export const POPULAR_TOTAL_TRACKS = playlistedTrackIds.size - 1;
