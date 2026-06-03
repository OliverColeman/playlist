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

# Test
cargo test --workspace
```

## Styling

Styling uses Tailwind CSS plus a SCSS stylesheet. As of Dioxus 0.7, Tailwind is built
automatically by `dx serve` — no manual CLI install needed. The input/output paths are
configured in [Dioxus.toml](Dioxus.toml):

```toml
[application]
tailwind_input = "crates/web/tailwind.css"
tailwind_output = "crates/web/assets/tailwind.css"
```
