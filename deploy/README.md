# Deploying with Docker Compose

Runs the Just Dance Archives web app and a MongoDB instance as containers on a
VPS. The same image also contains the `playlist-cli` tool, so you can run imports
and migrations by exec-ing into the running web container.

This directory holds:

- **`Dockerfile`** — builds the app image. GitHub Actions builds from it and
  pushes to GHCR (see *Building the image* below); you don't build it by hand.
- **`vps/`** — everything that runs on the server: the compose stack (Caddy +
  web + MongoDB), the Caddy config, and the setup/run scripts.

For local development of the app itself, use `dev/run_local/` (`dx serve`), not
this directory.

---

## VPS deployment (`deploy/vps/`)

Everything needed on the server lives in `deploy/vps/`:

| File | Purpose |
| --- | --- |
| `compose.yaml` | Pulls the GHCR image; runs Caddy + web + MongoDB. |
| `Caddyfile` | Reverse-proxy config (automatic HTTPS). |
| `.playlist.env.example` | Template for the env file the scripts load. |
| `start.sh` / `stop.sh` | Pull + (re)start, and stop. |
| `remote_playlist_cli.sh` | **Run from your local machine** — runs `playlist-cli` in the web container over SSH. |
| `install_docker.sh` | Install Docker Engine + Compose on Ubuntu/Debian. |
| `setup_ufw.sh` | Firewall: allow SSH, 80, 443. |
| `provision_vps.sh` | **Run from your local machine** — sets up a fresh VPS over SSH. |

### One-command provisioning (from your local machine)

`provision_vps.sh` does the whole first-run setup remotely over SSH: it copies
the files up, installs Docker, configures the firewall, ensures swap, and starts
the stack.

```bash
cd deploy/vps
cp .playlist.env.example .playlist.env   # edit: SITE_ADDRESS, DB_NAME, Spotify creds
# Point your domain's DNS A record at the VPS, then:
./provision_vps.sh root@<vps-ip>         # or [user@]host [-p PORT] [-d REMOTE_DIR]
```

The SSH user must be root or have sudo. Run it again any time to re-deploy
(it re-pulls the latest image and restarts). The manual steps below are the
equivalent if you'd rather set things up by hand on the box.

### First run on the VPS (manual)

```bash
cd deploy/vps

# One-time host setup
sudo ./install_docker.sh        # then log out/in (or `newgrp docker`)
sudo ./setup_ufw.sh

# Configure
cp .playlist.env.example .playlist.env   # edit: SITE_ADDRESS, DB_NAME, Spotify creds

# Start (pulls the latest image, then brings everything up)
./start.sh
```

`start.sh` / `stop.sh` run `docker compose` from this directory, so the
`compose.yaml` here is picked up automatically — no `-f` needed. They load
`.playlist.env` via `--env-file`.

### TLS and the public domain (Caddy)

`compose.yaml` here includes a **Caddy** reverse proxy that terminates HTTPS and
forwards to the web container. It's the only service that publishes ports
(80/443); the web app itself isn't exposed to the host.

To make it work:

1. Point your domain's DNS **A record** at the VPS's public IP.
2. Set `SITE_ADDRESS` (your domain) in `.playlist.env`.
3. Keep ports **80 and 443** open (`setup_ufw.sh` does this).

On first start Caddy automatically obtains a Let's Encrypt certificate for
`SITE_ADDRESS` and renews it. Certificates live in the `caddy_data` volume —
**don't delete it**, or you risk hitting Let's Encrypt rate limits on re-issue.
Port 80 must stay reachable for the ACME challenge and the HTTP→HTTPS redirect.

### Database migrations

```bash
cd deploy/vps
docker compose --env-file .playlist.env exec web playlist-cli dbmigrate
```

### Importing a playlist (the CLI / terminal access)

Run the CLI inside the running web container — it inherits the DB and Spotify
environment from compose:

```bash
docker compose --env-file .playlist.env exec web playlist-cli import \
    <playlist URI> [user_id] --name "<name>" --date YYYY-MM-DD
```

Or open an interactive shell in the container:

```bash
docker compose --env-file .playlist.env exec web bash
```

**From your local machine.** `remote_playlist_cli.sh` wraps the above over SSH so
you don't have to log in first — it forwards all arguments to `playlist-cli`:

```bash
cd deploy/vps
./remote_playlist_cli.sh import <playlist URI> [user_id] --name "<name>" --date YYYY-MM-DD

# Rename an existing compiler
./remote_playlist_cli.sh set-compiler-name <compiler_id> "<name>"
```

By default it SSHes to `SITE_ADDRESS` from `.playlist.env`. Override the
connection when needed (e.g. a specific SSH user, non-default port, or deploy dir
you passed to `provision_vps.sh`):

```bash
PLAYLIST_VPS=root@vps.example.com PLAYLIST_SSH_PORT=2222 PLAYLIST_REMOTE_DIR=/opt/playlist \
    ./remote_playlist_cli.sh dbmigrate
```

### Updating after a new image is published

```bash
cd deploy/vps
./start.sh        # re-pulls latest and restarts
```

### Logs and lifecycle

```bash
cd deploy/vps
docker compose --env-file .playlist.env logs -f web   # follow web server logs
docker compose --env-file .playlist.env ps            # status
./stop.sh                                             # stop (keeps data volume)
docker compose --env-file .playlist.env down -v       # stop AND delete volumes
```

MongoDB data persists in the `mongodb_data` named volume across restarts and
re-pulls. It is **not** published to the host — only the web container reaches it
over the internal compose network.

---

## Building the image (GitHub Actions → GHCR)

The [`build-image.yml`](../.github/workflows/build-image.yml) workflow builds the
image on GitHub's runners and pushes it to `ghcr.io/olivercoleman/playlist` on
every push to `main`, on `v*` tags, and on manual dispatch. It uses the built-in
`GITHUB_TOKEN` — no secrets to configure.

Tags published: `latest` (main), the branch name, git `v*` tags, and `sha-<commit>`.

**Make the VPS able to pull it.** GHCR packages start out private. Either:

- Make the package public — on GitHub: *Packages → playlist → Package settings →
  Change visibility → Public*. Then the VPS can `docker pull` with no login. Or
- Keep it private and log in on the VPS with a personal access token that has
  `read:packages`:
  ```bash
  echo "$GHCR_PAT" | docker login ghcr.io -u OliverColeman --password-stdin
  ```

Free-tier notes: GitHub Actions minutes are unlimited for public repos (2,000
min/month for private), and GHCR storage/bandwidth is free for public packages.

---

## Backups

```bash
cd deploy/vps
# Dump the database (reads DB_NAME from .playlist.env)
docker compose --env-file .playlist.env exec -T mongodb \
    mongodump --archive --db "$(grep -E '^DB_NAME=' .playlist.env | cut -d= -f2)" > backup.archive

# Restore
docker compose --env-file .playlist.env exec -T mongodb mongorestore --archive < backup.archive
```

## Notes / hardening for production

- **MongoDB auth is off** for a simple start. Since the DB port isn't exposed to
  the host this is acceptable behind a firewall, but for production enable auth:
  set `MONGO_INITDB_ROOT_USERNAME` / `MONGO_INITDB_ROOT_PASSWORD` on the
  `mongodb` service and update `DB_CONNECTION_STRING` to
  `mongodb://user:pass@mongodb:27017/?authSource=admin`.

### Sizing for a 1 GB VPS

- **Build off-box.** The Rust + wasm compile needs ~2 GB+ RAM, so don't build on
  the VPS — use the prebuilt GHCR image (which `deploy/vps/compose.yaml` does).
- **MongoDB cache** is capped at `0.4 GB`. Rough runtime budget: ~0.4 GB Mongo
  cache + ~0.2 GB Mongo overhead + ~0.08 GB web server + ~0.2 GB OS/Docker ≈
  0.9 GB, leaving a small margin.
- **Add a swapfile** as insurance against spikes (cheap, recommended):
  ```bash
  fallocate -l 1G /swapfile && chmod 600 /swapfile && mkswap /swapfile && swapon /swapfile
  echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
  ```
- If you later move to a larger VPS, raise `--wiredTigerCacheSizeGB` in
  `deploy/vps/compose.yaml` (rule of thumb: ~50% of RAM above the first 1 GB).
