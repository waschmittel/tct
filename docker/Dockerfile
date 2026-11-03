# syntax=docker/dockerfile:1

# ── Stage 1: Build tct binary ─────────────────────────────────────────────────
FROM rust:1-bookworm AS builder

WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs ./
COPY .git ./.git
COPY vendor ./vendor
COPY assets ./assets
COPY src ./src

RUN cargo build --release --locked

# ── Stage 2: Runtime (tct + ttyd + nginx PWA shell) ───────────────────────────
FROM debian:bookworm-slim

ARG TTYD_VERSION=1.7.7

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        nginx-light \
        openssl \
        tini \
    && rm -rf /var/lib/apt/lists/* \
    && rm -f /etc/nginx/sites-enabled/default

# Install official static ttyd binary (supports x86_64 and aarch64)
RUN ARCH="$(uname -m)" && \
    curl -fsSL "https://github.com/tsl0922/ttyd/releases/download/${TTYD_VERSION}/ttyd.${ARCH}" \
        -o /usr/local/bin/ttyd && \
    chmod +x /usr/local/bin/ttyd

COPY --from=builder /src/target/release/tct /usr/local/bin/tct

# Copy PWA static files, nginx configuration, and startup/build scripts
RUN mkdir -p /usr/share/tct-pwa /etc/nginx/snippets /data
COPY docker/pwa/manifest.webmanifest /usr/share/tct-pwa/manifest.webmanifest
COPY docker/pwa/sw.js /usr/share/tct-pwa/sw.js
COPY docker/pwa/tct-pwa.js /usr/share/tct-pwa/tct-pwa.js
COPY docker/pwa/icon.svg /usr/share/tct-pwa/icon.svg
COPY docker/pwa/head.html /usr/share/tct-pwa/head.html
COPY docker/nginx.conf /etc/nginx/conf.d/tct.conf
COPY docker/entrypoint.sh /usr/local/bin/entrypoint.sh
COPY docker/build-index.sh /tmp/build-index.sh

RUN chmod +x /usr/local/bin/entrypoint.sh /tmp/build-index.sh && \
    /tmp/build-index.sh /usr/share/tct-pwa/head.html /usr/share/tct-pwa/index.html && \
    rm -f /tmp/build-index.sh

ENV TCT_DATA_DIR=/data \
    TERM=xterm-256color \
    COLORTERM=truecolor

VOLUME ["/data"]
EXPOSE 7681

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]
