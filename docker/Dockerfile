# syntax=docker/dockerfile:1

# ── Stage 1: Build tct binary ─────────────────────────────────────────────────
# Base images are pinned by digest (tag kept for readability); bump both together.
FROM rust:1-bookworm@sha256:ade39a710ef85f3506cffcfec3e8a24ba090badef1dff2fb798f71d878833fea AS builder

WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs ./
COPY .git ./.git
COPY vendor ./vendor
COPY assets ./assets
COPY README.md ./README.md
COPY docs/user-guide.md ./docs/user-guide.md
COPY src ./src

RUN cargo build --release --locked

# ── Stage 2: Runtime (tct + ttyd + nginx PWA shell) ───────────────────────────
FROM debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc

ARG TTYD_VERSION=1.7.7

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        nginx-light \
        openssl \
        tini \
    && rm -rf /var/lib/apt/lists/* \
    && rm -f /etc/nginx/sites-enabled/default

# Install official static ttyd binary (supports x86_64 and aarch64), verified
# against the checksums of the pinned release.
ARG TTYD_SHA256_X86_64=8a217c968aba172e0dbf3f34447218dc015bc4d5e59bf51db2f2cd12b7be4f55
ARG TTYD_SHA256_AARCH64=b38acadd89d1d396a0f5649aa52c539edbad07f4bc7348b27b4f4b7219dd4165
RUN ARCH="$(uname -m)" && \
    case "$ARCH" in \
        x86_64) SUM="$TTYD_SHA256_X86_64" ;; \
        aarch64) SUM="$TTYD_SHA256_AARCH64" ;; \
        *) echo "unsupported architecture: $ARCH" >&2; exit 1 ;; \
    esac && \
    curl -fsSL "https://github.com/tsl0922/ttyd/releases/download/${TTYD_VERSION}/ttyd.${ARCH}" \
        -o /usr/local/bin/ttyd && \
    echo "${SUM}  /usr/local/bin/ttyd" | sha256sum -c - && \
    chmod +x /usr/local/bin/ttyd

COPY --from=builder /src/target/release/tct /usr/local/bin/tct

# Copy PWA static files, nginx configuration, and startup/build scripts
RUN mkdir -p /usr/share/tct-pwa /data
COPY docker/pwa/manifest.webmanifest /usr/share/tct-pwa/manifest.webmanifest
COPY docker/pwa/sw.js /usr/share/tct-pwa/sw.js
COPY docker/pwa/tct-pwa.js /usr/share/tct-pwa/tct-pwa.js
COPY docker/pwa/icon.svg /usr/share/tct-pwa/icon.svg
COPY docker/pwa/head.html /usr/share/tct-pwa/head.html
COPY docker/nginx.conf /etc/nginx/tct-nginx.conf
COPY docker/nginx-headers.conf /etc/nginx/tct-headers.conf
COPY docker/entrypoint.sh /usr/local/bin/entrypoint.sh
COPY docker/build-index.sh /tmp/build-index.sh

RUN chmod +x /usr/local/bin/entrypoint.sh /tmp/build-index.sh && \
    /tmp/build-index.sh /usr/share/tct-pwa/head.html /usr/share/tct-pwa/index.html && \
    rm -f /tmp/build-index.sh

# Run as an unprivileged user: nginx listens on 7681 (> 1024) and keeps its state in /tmp/tct.
# Bind-mounted data dirs must be writable by this uid (or pass `--user`).
RUN useradd --uid 10001 --user-group --no-create-home --shell /usr/sbin/nologin tct && \
    chown tct:tct /data
USER tct

ENV TCT_DATA_DIR=/data \
    TERM=xterm-256color \
    COLORTERM=truecolor

VOLUME ["/data"]
EXPOSE 7681

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]
