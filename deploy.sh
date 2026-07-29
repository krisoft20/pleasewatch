#!/usr/bin/env bash
# v1-pattern deploy: build frontend locally, ship only the build artifact.
# backend still builds on the VPS (cross-compile from Windows is painful).
# only ships the dirs that actually changed.

set -euo pipefail

DEPLOY_ENV="${DEPLOY_ENV:-.deploy.env}"
if [ -f "$DEPLOY_ENV" ]; then
    set -a
    . "$DEPLOY_ENV"
    set +a
fi

VPS_HOST="${VPS_HOST:-}"
VPS_DIR="${VPS_DIR:-/opt/pleasewatch}"
VPS_URL="${VPS_URL:-}"
COMPOSE="${COMPOSE:-docker-compose.simple.yml}"
HASH_DIR=".deploy-hashes"
mkdir -p "$HASH_DIR"

if [ -z "$VPS_HOST" ]; then
    echo "set VPS_HOST=user@host before running deploy.sh"
    exit 1
fi

hash_dir() {
  { find "$@" -type f 2>/dev/null | sort | xargs sha256sum 2>/dev/null | sha256sum | cut -d' ' -f1; } || true
}

FRONTEND_HASH=$(hash_dir frontend/src frontend/package.json frontend/package-lock.json frontend/svelte.config.js frontend/vite.config.ts frontend/tailwind.config.js)
BACKEND_HASH=$(hash_dir backend/src backend/Cargo.toml backend/Cargo.lock backend/.cargo)
DEPLOY_HASH=$(hash_dir deploy/Caddyfile deploy/docker-compose.simple.yml deploy/docker-compose.yml backend/Dockerfile backend/Dockerfile.builder backend/scripts)

NEED_FRONTEND=1; NEED_BACKEND=1; NEED_DEPLOY=1
if [ -f "$HASH_DIR/frontend" ] && [ "$(cat "$HASH_DIR/frontend")" = "$FRONTEND_HASH" ]; then NEED_FRONTEND=0; fi
if [ -f "$HASH_DIR/backend"  ] && [ "$(cat "$HASH_DIR/backend")"  = "$BACKEND_HASH"  ]; then NEED_BACKEND=0;  fi
if [ -f "$HASH_DIR/deploy"   ] && [ "$(cat "$HASH_DIR/deploy")"   = "$DEPLOY_HASH"   ]; then NEED_DEPLOY=0;   fi

echo "-> changed: frontend=$NEED_FRONTEND backend=$NEED_BACKEND deploy=$NEED_DEPLOY"

if [ "$NEED_FRONTEND" = "1" ]; then
    echo "-> building frontend locally..."
    pushd frontend > /dev/null
    if [ ! -d node_modules ]; then
        echo "-> first run, npm ci..."
        npm ci --prefer-offline --no-audit --no-fund 2>&1 | tail -3
    elif ! cmp -s package-lock.json node_modules/.lock-stamp 2>/dev/null; then
        echo "-> lockfile changed, npm ci..."
        npm ci --prefer-offline --no-audit --no-fund 2>&1 | tail -3
        cp package-lock.json node_modules/.lock-stamp
    fi
    [ ! -f node_modules/.lock-stamp ] && cp package-lock.json node_modules/.lock-stamp 2>/dev/null || true
    echo "-> npm run build..."
    npm run build 2>&1 | tail -3
    popd > /dev/null
fi

ssh -o ServerAliveInterval=30 "$VPS_HOST" "mkdir -p $VPS_DIR $VPS_DIR/deploy/static $VPS_DIR/deploy/bin"

if [ "$NEED_FRONTEND" = "1" ]; then
    echo "-> shipping frontend build artifact..."
    tar -C frontend/build -czf - . | ssh -o ServerAliveInterval=30 "$VPS_HOST" "mkdir -p $VPS_DIR/deploy/static && find $VPS_DIR/deploy/static -mindepth 1 -delete && tar -C $VPS_DIR/deploy/static -xzf -"
fi

if [ "$NEED_BACKEND" = "1" ]; then
    echo "-> shipping backend source..."
    # -m: stamp extraction time, otherwise cargo sees mtimes older than its
    # fingerprint and skips the rebuild (edit-during-deploy bites here)
    tar --exclude='backend/target' --exclude='backend/.cargo/registry' --exclude='backend/data' -czf - backend | ssh -o ServerAliveInterval=30 "$VPS_HOST" "tar -C $VPS_DIR -m -xzf -"
fi

if [ "$NEED_DEPLOY" = "1" ]; then
    echo "-> shipping deploy + dockerfile changes..."
    tar -czf - \
      deploy/Caddyfile \
      deploy/docker-compose.simple.yml \
      deploy/docker-compose.yml \
      deploy/setup.sh \
      deploy/.env.example \
      deploy/jackett-expose.sh \
      deploy/prowlarr-expose.sh \
      backend/Dockerfile \
      backend/Dockerfile.builder \
      backend/scripts \
      | ssh -o ServerAliveInterval=30 "$VPS_HOST" "mkdir -p $VPS_DIR && tar -C $VPS_DIR -xzf -"
fi

ssh -o ServerAliveInterval=30 "$VPS_HOST" "set -eo pipefail; cd $VPS_DIR/deploy && \
    if [ ! -f .env ]; then cp .env.example .env && echo '[!] created .env from example. fill it in and re-run.' && exit 1; fi && \
    mkdir -p media qbit-config jackett-config prowlarr-config pw-data bin static && \
    if [ '$NEED_DEPLOY' = '1' ] || ! docker image inspect pleasewatch-runtime:latest > /dev/null 2>&1; then \
      echo '-> building runtime + builder images...' && \
      docker build -q -f $VPS_DIR/backend/Dockerfile -t pleasewatch-runtime:latest $VPS_DIR/backend > /dev/null && \
      docker build -q -f $VPS_DIR/backend/Dockerfile.builder -t pw-rust-builder:latest $VPS_DIR/backend > /dev/null; \
    fi && \
    NEED_RESTART=0 && \
    if [ '$NEED_BACKEND' = '1' ] || [ ! -f bin/pleasewatch ]; then \
      echo '-> cargo build (mold + incremental)...' && \
      docker run --rm \
        -v $VPS_DIR/backend:/src \
        -v pw-cargo-target:/target \
        -v pw-cargo-registry:/usr/local/cargo/registry \
        -e CARGO_TARGET_DIR=/target \
        -w /src \
        pw-rust-builder:latest sh -c 'cargo build --release --offline || cargo build --release --locked' 2>&1 | tail -5 && \
      docker run --rm \
        -v pw-cargo-target:/target:ro \
        -v $VPS_DIR/deploy/bin:/host \
        alpine sh -c 'cp /target/release/pleasewatch /host/pleasewatch.new && mv /host/pleasewatch.new /host/pleasewatch && chmod +x /host/pleasewatch' && \
      NEED_RESTART=1; \
    fi && \
    OVERRIDE_ARG='' && \
    if [ -f docker-compose.override.yml ]; then OVERRIDE_ARG='-f docker-compose.override.yml'; fi && \
    UP_OUT=\$(docker compose -f $COMPOSE \$OVERRIDE_ARG up -d 2>&1) && \
    echo \"\$UP_OUT\" | tail -3 && \
    if [ \"\$NEED_RESTART\" = 1 ]; then \
      if echo \"\$UP_OUT\" | grep -Eq 'pleasewatch-pleasewatch-1.*(Started|Recreated|Created)'; then \
        echo '-> pleasewatch already restarted'; \
      else \
        echo '-> restart pleasewatch (new binary)' && \
        docker compose -f $COMPOSE \$OVERRIDE_ARG up -d --no-deps --force-recreate pleasewatch 2>&1 | tail -3; \
      fi; \
    else \
      echo '-> binary unchanged, no restart'; \
    fi"

echo "$FRONTEND_HASH" > "$HASH_DIR/frontend"
echo "$BACKEND_HASH"  > "$HASH_DIR/backend"
echo "$DEPLOY_HASH"   > "$HASH_DIR/deploy"

if [ -n "$VPS_URL" ]; then
    echo "-> smoke test..."
    sleep 3
    curl -ksS -o /dev/null -w '%{http_code}\n' "$VPS_URL/" || echo "(no response yet)"
else
    echo "-> no VPS_URL set, skipping smoke test"
fi
echo "deployed."
