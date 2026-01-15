# Playlist Workspace

This project is structured as a Cargo workspace with three crates:

## Crates

### 1. `playlist-core` (Library)
Located in `crates/core/`

Shared library containing:
- **Database configuration and connection** (`config.rs`, `database.rs`)
- **Data models** (`models/` - Album, Artist, Compiler, Playlist, Track, etc.)
- **Common error types** (`ServerError`)

This crate is used by both the web app and CLI tools.

### 2. `playlist-web` (Binary)
Located in `crates/web/`

The Dioxus web application with:
- **Frontend components** (Hero, etc.)
- **Views/pages** (Home, Playlist, Compiler, Track, Artist, Popular)
- **Routing** (Dioxus Router)
- **Server functions** (when built with `server` feature)

### 3. `playlist-cli` (Binary)
Located in `crates/cli/`

Command-line tools including:
- **Migration commands** (`commands/migrate.rs`)
- Database maintenance tasks
- Other CLI utilities

## Running the Application

### Web App
```bash
# From project root
./dev/run_local/run.sh

# Or manually:
dx serve -p playlist-web
```

**Note**: Assets (CSS, images, etc.) are located in `crates/web/assets/`. If you add new assets to the workspace root `assets/` folder, copy them to `crates/web/assets/` as well.

### CLI Tools
```bash
# Run migrations
cargo run -p playlist-cli

# Or build and run:
cargo build -p playlist-cli --release
./target/release/playlist-cli
```

## Development

### Building
```bash
# Build everything
cargo build --workspace

# Build specific crate
cargo build -p playlist-web
cargo build -p playlist-cli
cargo build -p playlist-core
```

### Testing
```bash
# Test all crates
cargo test --workspace

# Test specific crate
cargo test -p playlist-core
```

### Features

The crates support different feature flags:

- **playlist-core**: `server` (enables dioxus server functions)
- **playlist-web**: `web` (default), `server` (for fullstack mode)

## Environment Variables

Both web and CLI require:
- `DB_CONNECTION_STRING` - MongoDB connection string
- `DB_NAME` - Database name

Set these in `dev/run_local/.local.env` for local development.
