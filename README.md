# pleasewatch

pleasewatch is a self-hosted media library and watch app. It has a Rust/Axum backend, SQLite storage, a SvelteKit frontend, and a Docker deploy stack with Caddy, qBittorrent, Jackett, and Prowlarr.

The project is built for private libraries and media you have the right to access. The torrent/indexer pieces are integrations, not a license to fetch copyrighted content.

## what it does

- movie and tv search through TMDB
- library pages for movies, shows, books, manga, and anime
- continue watching, watch progress, collections, recommendations, and recently added shelves
- browser video player with audio/subtitle picker, subtitle search/upload/sync, skip intro, next episode, clips, and watch together
- torrent picker backed by qBittorrent, Jackett, and Prowlarr
- first-run onboarding for TMDB and optional download services
- admin panel for users, pending approvals, downloads, clips, storage, logs, settings, health, and watch leaderboard
- sqlite-backed single binary backend, static frontend served by the backend in prod
- docker compose deployment with Caddy and the download services

## stack

- backend: Rust, Axum, rusqlite, Tokio, reqwest
- frontend: SvelteKit, Svelte 5, Vite, adapter-static
- storage: SQLite plus filesystem media folders
- deploy: Docker Compose, Caddy, qBittorrent, Jackett, Prowlarr

## local dev

Backend:
```sh
cd backend
TMDB_API_KEY=your_key cargo run
```

Frontend:
```sh
cd frontend
npm install
npm run dev
```

For local download testing, run Jackett/Prowlarr and qBittorrent separately, then configure their URLs and API keys in the admin settings.

## self-host

On a fresh Ubuntu/Debian VPS:
```sh
sudo apt-get update
sudo apt-get install -y curl git
curl -fsSL https://get.docker.com | sudo sh

sudo git clone https://github.com/krisoft20/pleasewatch /opt/pleasewatch
cd /opt/pleasewatch/deploy
```

If you forked the repo, use your fork URL instead.

For a real domain point DNS at the VPS first. Caddy handles HTTPS. Then run:
```sh
sudo ./setup.sh --domain watch.example.com
```

For a plain IP or LAN test:
```sh
sudo ./setup.sh
```

Open `http://your-server-ip`.

If you do not want to expose the test install yet, tunnel port 80:
```sh
ssh -L 8080:127.0.0.1:80 user@your-server-ip
```

Then open `http://127.0.0.1:8080`.

When setup finishes:
1. Open the URL printed by `setup.sh`.
2. Register the first account. The first account becomes admin.
3. Sign in.
4. Add a TMDB api key on `/onboarding`.
5. Open the app.

TMDB is the only required key for search and metadata. OMDB, Wyzie, qBittorrent, Jackett, and Prowlarr can be configured later in `/admin?tab=settings`.

## storage

Media is stored on the server filesystem. New installs default to `/opt/pleasewatch/media`, mounted as `/media` inside the containers.

For a real library, pick a VPS with actual storage or mount an extra disk first. Then set this in `deploy/.env` before running setup:

```sh
PW_MEDIA_DIR=/mnt/storage/pleasewatch
```

qBittorrent and pleasewatch both mount that path as `/media`, so downloads and processed files stay on the same disk.

To update:

```sh
cd /opt/pleasewatch
sudo git pull
cd deploy
sudo ./setup.sh
```

The setup script keeps the existing `.env`, database, and media folders.

Useful checks:

```sh
cd /opt/pleasewatch/deploy
sudo docker compose -f docker-compose.simple.yml ps
sudo docker compose logs -f pleasewatch
```

## deploy from local machine

`deploy.sh` intentionally has no hardcoded host. Copy the local deploy template and fill it with your own host:

```sh
cp .deploy.env.example .deploy.env
```

Then run:

```sh
./deploy.sh
```

On Windows with Git Bash:

```sh
"C:\Program Files\Git\bin\bash.exe" deploy.sh
```

## credits

Movie and TV metadata and images are provided by [TMDB](https://www.themoviedb.org/).
This product uses the TMDB API but is not endorsed or certified by TMDB.

Other notices are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## license

The source code is licensed under the [GNU Affero General Public License v3.0 only](LICENSE) (`AGPL-3.0-only`).

The PleaseWatch name and logo are not covered by that license. See [TRADEMARKS.md](TRADEMARKS.md).
