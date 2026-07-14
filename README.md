# The Just Dance Playlist Archives

A web app that archives the music playlists from **Just Dance**, a free-movement, loosely
facilitated dance session in Newcastle, Australia. It lets you browse playlists, tracks,
artists, and the people who compile the sessions, with links out to streaming services.

Playlists are imported from music services. The service is backed by MongoDB. The frontend 
is a [Dioxus 0.7](https://dioxuslabs.com/learn/0.7) fullstack app (server-rendered web,
hydrated on the client).

## Workspace layout

This is a Cargo workspace with three crates:

```
crates/
├─ core/  # playlist-core — shared library: config, MongoDB access, data models, errors
├─ web/   # playlist-web  — Dioxus fullstack web app (UI + server functions)
└─ cli/   # playlist-cli  — command-line tools: music service import, DB migrations
```

### `playlist-core`
Shared library used by both the web app and the CLI. Contains the database
configuration and connection ([config.rs](crates/core/src/config.rs),
[database.rs](crates/core/src/database.rs)), the data models
([models/](crates/core/src/models/) — Album, Artist, Compiler, Playlist, Track), and
common error types. Server-only functionality (MongoDB, indexing) is gated behind the
`server` feature.

### `playlist-web`
The Dioxus web application. Routes are defined in [main.rs](crates/web/src/main.rs):

| Route | View |
| --- | --- |
| `/` | Home — list of playlists |
| `/playlist/:id` | A single playlist |
| `/compiler` and `/compiler/:id` | Playlist compilers |
| `/track/:id` | A single track |
| `/artist/:id` | A single artist |
| `/popular` | Popular tracks |
| `/search` | Search (artist, playlist, compiler) |

Server functions live in [api.rs](crates/web/src/api.rs) and run only when built with the
`server` feature. The default feature is `web` (client/wasm).

### `playlist-cli`
Command-line tools ([main.rs](crates/cli/src/main.rs)):

- `dbmigrate` — run database migrations.
- `import <playlist URI> [user_id] [--name <name>] [--date <YYYY-MM-DD>]` — import a
  playlist from a music service.

Playlist dates are interpreted and displayed in the canonical `Australia/Sydney` timezone
(`playlist_core::TIMEZONE` — the sessions happen in Newcastle, Australia): the `--date`
value is parsed as midnight in that zone and the web UI formats dates in it, regardless of
the host machine's timezone.

## Environment variables

Both the web app and CLI require a MongoDB connection:

- `DB_CONNECTION_STRING` — MongoDB connection string
- `DB_NAME` — database name

The CLI `import` command additionally needs music service client-credentials, eg for Spotify:

- `SPOTIFY_CLIENT_ID` / `SPOTIFY_CLIENT_SECRET`
- `SPOTIFY_MARKET` — optional, defaults to `AU`

For local development these are read from `dev/run_local/.local.env` (gitignored).

## Running locally

The helper scripts in [dev/run_local/](dev/run_local/) start the required external
services (MongoDB via Docker Compose) and source `.local.env` before running.

```bash
# Serve the web app at http://localhost:8080
dev/run_local/runweb.sh

# Run database migrations
dev/run_local/runmigrate.sh

# Import a Spotify playlist
dev/run_local/runimport.sh <playlist URI> [user_id] [--name <name>] [--date YYYY-MM-DD]
```

To run things manually instead (with the env vars set yourself):

```bash
dx serve -p playlist-web                 # web app
cargo run -p playlist-cli dbmigrate      # migrations
cargo run -p playlist-cli import <uri>   # import
```

## Building and testing

```bash
# Build everything
cargo build --workspace

# Build a specific crate
cargo build -p playlist-web
cargo build -p playlist-cli
```

Tests are organised in three layers, with a runner script for each in
[dev/test/](dev/test/):

```bash
dev/test/run_unit.sh         # unit tests (no external services needed)
dev/test/run_integration.sh  # database integration tests (needs MongoDB)
dev/test/run_e2e.sh          # Playwright end-to-end tests (needs MongoDB + Node)
dev/test/run_all.sh          # all of the above, in sequence
```

**MongoDB requirement.** The integration and e2e layers need a local MongoDB at
`mongodb://localhost:27017`. The runner scripts probe that port first and reuse
whatever is already listening (e.g. a CI service container); only if nothing is
listening do they start MongoDB via Docker Compose
([dev/run_local/external_services.docker_compose.yaml](dev/run_local/external_services.docker_compose.yaml)).
Each layer uses its own reserved database(s), which it drops/cleans at the start of a
run so runs are idempotent:

| Layer | Database(s) |
| --- | --- |
| core integration tests | `playlist_test_core` |
| cli integration tests | `playlist_test_cli_*` — one suffixed database per test, e.g. `playlist_test_cli_import`, `playlist_test_cli_migrate` |
| e2e tests | `playlist_e2e` |

Test databases are cleaned at the *start* of the next run, not at exit, so they are
left behind afterwards. To drop them all (adjust `docker exec mongodb` to plain
`mongosh` for a native MongoDB):

```bash
docker exec mongodb mongosh --quiet --eval 'db.getMongo().getDBNames()
  .filter(n => n.startsWith("playlist_test_") || n.startsWith("playlist_e2e"))
  .forEach(n => db.getSiblingDB(n).dropDatabase())'
```

**Unit tests** run with plain `cargo test` across the workspace, plus the
server-feature test configurations:

```bash
cargo test --workspace
cargo test -p playlist-core --features server
cargo test -p playlist-web --features server
```

**End-to-end tests** live in [e2e/](e2e/) (Playwright + TypeScript). They seed the
`playlist_e2e` database with fixtures (see `e2e/fixtures/data.ts`), start the
pre-built fullstack server binary (`target/dx/playlist-web/debug/web/playlist-web`)
on port 8811, and exercise the rendered pages and the HTTP API.
`dev/test/run_e2e.sh` needs only Node.js (22+) — it installs the npm dependencies and
the Playwright Chromium browser itself, and (re)builds the server binary with
`dx build -p playlist-web --fullstack` when it is missing or older than the sources
under `crates/web/src`, `crates/core/src` or `crates/web/assets` (set `SKIP_BUILD=1`
to use the existing binary as-is):

```bash
dev/test/run_e2e.sh                            # whole suite
dev/test/run_e2e.sh tests/search.spec.ts       # a single spec
dev/test/run_e2e.sh --headed                   # watch the browser
SKIP_BUILD=1 dev/test/run_e2e.sh               # skip the binary freshness check
```

Because the e2e suite *drops and re-seeds* its database, it refuses to run against
any database whose name does not start with `playlist_e2e` (see `e2e/db-config.ts`) —
ambient `DB_NAME`/`DB_CONNECTION_STRING` values from your shell cannot silently
redirect it at a real database. To deliberately target a different MongoDB instance
or database, set `E2E_DB_CONNECTION_STRING` and/or `E2E_DB_NAME` (the
`playlist_e2e` name prefix is still enforced).

## Styling

Styling uses Tailwind CSS plus a SCSS stylesheet. As of Dioxus 0.7, Tailwind is built
automatically by `dx serve` — no manual CLI install needed. The input/output paths are
configured in [Dioxus.toml](Dioxus.toml):

```toml
[application]
tailwind_input = "crates/web/tailwind.css"
tailwind_output = "crates/web/assets/tailwind.css"
```
