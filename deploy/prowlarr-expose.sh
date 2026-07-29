#!/usr/bin/env bash
set -e
cd "$(dirname "$0")"

case "${1:-}" in
    on)
        cat > docker-compose.override.yml <<'OVR'
services:
  prowlarr:
    ports:
      - "127.0.0.1:9696:9696"
OVR
        docker compose up -d prowlarr
        echo "[+] prowlarr on 127.0.0.1:9696"
        echo "    remote? ssh -L 9696:localhost:9696 user@host"
        ;;
    off)
        rm -f docker-compose.override.yml
        docker compose up -d --force-recreate prowlarr
        echo "[+] prowlarr port closed, internal-only again"
        ;;
    *)
        echo "usage: $0 {on|off}"
        exit 1
        ;;
esac
