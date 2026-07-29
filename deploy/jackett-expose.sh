#!/usr/bin/env bash
set -e
cd "$(dirname "$0")"

case "${1:-}" in
    on)
        cat > docker-compose.override.yml <<'OVR'
services:
  jackett:
    ports:
      - "127.0.0.1:9117:9117"
OVR
        docker compose up -d jackett
        echo "[+] jackett on 127.0.0.1:9117"
        echo "    remote? ssh -L 9117:localhost:9117 user@host"
        ;;
    off)
        rm -f docker-compose.override.yml
        docker compose up -d --force-recreate jackett
        echo "[+] jackett port closed, internal-only again"
        ;;
    *)
        echo "usage: $0 {on|off}"
        exit 1
        ;;
esac
