#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

COMPOSE_FILE="docker-compose.simple.yml"
ROOT="$(cd .. && pwd)"
TMDB_ARG=""
DOMAIN_ARG=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --vpn) COMPOSE_FILE="docker-compose.yml" ;;
        --domain)
            shift
            if [ "$#" -eq 0 ]; then
                echo "[-] --domain needs a hostname"
                exit 1
            fi
            DOMAIN_ARG="$1"
            ;;
        --domain=*) DOMAIN_ARG="${1#*=}" ;;
        --tmdb)
            shift
            if [ "$#" -eq 0 ]; then
                echo "[-] --tmdb needs an api key"
                exit 1
            fi
            TMDB_ARG="$1"
            ;;
        --tmdb=*) TMDB_ARG="${1#*=}" ;;
        --*) ;;
        *) TMDB_ARG="$1" ;;
    esac
    shift
done

PW_PROJECT="${PW_PROJECT:-pleasewatch}"
PW_RUNTIME_IMAGE="${PW_RUNTIME_IMAGE:-pleasewatch-runtime:latest}"
PW_BUILDER_IMAGE="${PW_BUILDER_IMAGE:-pw-rust-builder:latest}"
PW_NODE_VOLUME="${PW_NODE_VOLUME:-pw-node-modules}"
PW_CARGO_TARGET_VOLUME="${PW_CARGO_TARGET_VOLUME:-pw-cargo-target}"
PW_CARGO_REGISTRY_VOLUME="${PW_CARGO_REGISTRY_VOLUME:-pw-cargo-registry}"
export PW_PROJECT PW_RUNTIME_IMAGE

command -v docker >/dev/null 2>&1 || {
    echo "[-] docker not installed. install: curl -fsSL https://get.docker.com | sh"
    exit 1
}
docker compose version >/dev/null 2>&1 || {
    echo "[-] docker compose v2 missing. install the compose plugin."
    exit 1
}

sed_inplace() {
    if sed --version >/dev/null 2>&1; then
        sed -i "$1" "$2"
    else
        sed -i '' "$1" "$2"
    fi
}

set_env() {
    key="$1"
    val="$2"
    if grep -q "^$key=" .env; then
        sed_inplace "s|^$key=.*|$key=$val|" .env
    else
        echo "$key=$val" >> .env
    fi
}

if [ ! -f .env ]; then
    cp .env.example .env
fi

env_media_dir=$(grep '^PW_MEDIA_DIR=' .env | cut -d= -f2- || true)
PW_MEDIA_DIR="${PW_MEDIA_DIR:-$env_media_dir}"
PW_MEDIA_DIR="${PW_MEDIA_DIR:-./media}"
export PW_MEDIA_DIR

if [ -n "$DOMAIN_ARG" ]; then
    set_env "PW_HOSTNAME" "$DOMAIN_ARG"
    if [ -z "${PUBLIC_BASE_URL:-}" ]; then
        set_env "PUBLIC_BASE_URL" "https://$DOMAIN_ARG"
    fi
fi

for key in PW_HOSTNAME PUBLIC_BASE_URL CADDY_HTTP_PORT CADDY_HTTPS_PORT QBIT_BIND QBIT_P2P_PORT PW_MEDIA_DIR WYZIE_API_KEY; do
    val="${!key:-}"
    if [ -n "$val" ]; then
        set_env "$key" "$val"
    fi
done

if ! grep -q '^QBIT_PASS=..' .env; then
    qbpass=$(openssl rand -base64 18 | tr -d '/=+' | head -c 24)
    set_env "QBIT_PASS" "$qbpass"
fi

current_tmdb=$(grep '^TMDB_API_KEY=' .env | cut -d= -f2- || true)

if [ -n "$TMDB_ARG" ]; then
    set_env "TMDB_API_KEY" "$TMDB_ARG"
elif [ -n "${TMDB_API_KEY:-}" ] && [ "$current_tmdb" != "$TMDB_API_KEY" ]; then
    set_env "TMDB_API_KEY" "$TMDB_API_KEY"
fi

current_tmdb=$(grep '^TMDB_API_KEY=' .env | cut -d= -f2- || true)
if [ -z "$current_tmdb" ]; then
    echo "[+] TMDB_API_KEY empty, first-run onboarding will ask for it in the browser"
fi

mkdir -p pw-data jackett-config prowlarr-config qbit-config caddy-data caddy-config "$PW_MEDIA_DIR" gluetun-config
mkdir -p bin static

if [ "$(id -u)" = "0" ]; then
    modprobe wireguard 2>/dev/null && echo "[+] kernel wireguard module loaded"
fi

echo "[+] building runtime image..."
docker build -q -f "$ROOT/backend/Dockerfile" -t "$PW_RUNTIME_IMAGE" "$ROOT/backend" > /dev/null

echo "[+] building frontend..."
docker run --rm \
    -v "$ROOT/frontend:/src" \
    -v "$PW_NODE_VOLUME:/src/node_modules" \
    -w /src \
    node:22-bookworm \
    sh -lc 'npm ci --no-audit --no-fund && npm run build'
find static -mindepth 1 -delete
cp -a "$ROOT/frontend/build/." static/

echo "[+] building backend..."
docker build -q -f "$ROOT/backend/Dockerfile.builder" -t "$PW_BUILDER_IMAGE" "$ROOT/backend" > /dev/null
docker run --rm \
    -v "$ROOT/backend:/src" \
    -v "$PW_CARGO_TARGET_VOLUME:/target" \
    -v "$PW_CARGO_REGISTRY_VOLUME:/usr/local/cargo/registry" \
    -e CARGO_TARGET_DIR=/target \
    -w /src \
    "$PW_BUILDER_IMAGE" cargo build --release
docker run --rm \
    -v "$PW_CARGO_TARGET_VOLUME:/target:ro" \
    -v "$PWD/bin:/host" \
    alpine sh -c 'cp /target/release/pleasewatch /host/pleasewatch.new && mv /host/pleasewatch.new /host/pleasewatch && chmod +x /host/pleasewatch'

echo "[+] starting stack with $COMPOSE_FILE..."
docker compose -p "$PW_PROJECT" -f "$COMPOSE_FILE" up -d

public_url=$(grep '^PUBLIC_BASE_URL=' .env | cut -d= -f2- || true)
host=$(grep '^PW_HOSTNAME=' .env | cut -d= -f2- || true)
if [ -n "$public_url" ]; then
    open_url="$public_url"
elif [ "$host" = ":80" ] || [ -z "$host" ]; then
    open_url="http://your-server-ip"
elif [[ "$host" == :* ]]; then
    open_url="http://your-server-ip$host"
else
    open_url="https://$host"
fi

cat <<EOF

[+] up.

    open:
      $open_url

    first browser steps:
      1. create the first account. it becomes admin.
      2. sign in.
      3. add a TMDB api key when /onboarding asks for it.

    optional later:
      - qBittorrent, Jackett, and Prowlarr status live in /admin -> settings.
      - vpn can be enabled from /onboarding or /admin -> settings.
      - setup generated .env values for secrets and qbit password.
EOF
