FROM rust:1.97-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends mold clang \
    && rm -rf /var/lib/apt/lists/*
