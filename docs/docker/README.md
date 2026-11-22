# Self-Hosting `tct` as a Browser PWA (Docker & OAuth)

Each `tct` release publishes a multi-arch (`linux/amd64`, `linux/arm64`) Docker image to `ghcr.io/waschmittel/tct` that serves `tct` in the browser as an installable **Progressive Web App (PWA)**.

## What's Inside the Container

- **`tct`** running inside [`ttyd`](https://github.com/tsl0922/ttyd) (`xterm.js` over WebSockets) with 24-bit color (`COLORTERM=truecolor`) and **OSC 52** clipboard support (pressing `y` in the TUI copies directly to your local device clipboard).
- **`nginx-light`** on port `7681` serving the PWA manifest (`/manifest.webmanifest`), service worker (`/sw.js`), and vector icon (`/icon.svg`), while proxying `/ws` and `/token` to `ttyd`.
- **Opt-in HTTP Basic Auth** via `TCT_BASIC_AUTH_USER` and `TCT_BASIC_AUTH_PASSWORD` for standalone deployments without an external auth proxy.

- **PDF export**: `p` (black and white) and `P` (color) send the card's PDF to your browser as a download — or the share sheet on phones and tablets. Nothing is written on the server and there is no save dialog. The image sets `TCT_WEB=1` to switch this on; `tct` then streams the PDF through the terminal as a private escape sequence (`OSC 7733`, see [`src/export/web.rs`](../../src/export/web.rs)) that [`tct-pwa.js`](../../docker/pwa/tct-pwa.js) turns into the download. PDFs over 25 MB are refused.

---

## Option 1: Standalone Container (Local or Opt-In Basic Auth)

### Localhost / Tailscale (No Built-in Auth)

If you are running locally or behind a private network/SSO proxy that already handles authentication, bind to `127.0.0.1` (or your private interface):

```sh
docker run -d \
  --name tct \
  -p 127.0.0.1:7681:7681 \
  -v ~/.tct:/data \
  ghcr.io/waschmittel/tct:latest
```

The image runs as an unprivileged user (uid 10001), so the mounted directory must be writable by it — or run as the directory's owner with `--user "$(id -u):$(id -g)"` (add `--read-only --tmpfs /tmp --cap-drop ALL --security-opt no-new-privileges` to harden further). Pin a release tag instead of `latest` for deliberate upgrades.

The terminal WebSocket verifies the browser's `Origin` against the `Host` header (`ttyd --check-origin`), so other websites cannot drive a session on `localhost`. A reverse proxy must pass the original `Host` through; if yours rewrites it, set `-e TCT_CHECK_ORIGIN=0` and enforce the origin there instead.

### Standalone with Opt-In Basic Auth

Set `TCT_BASIC_AUTH_USER` and `TCT_BASIC_AUTH_PASSWORD` to enable HTTP Basic Auth directly inside the container without needing a second container:

```sh
docker run -d \
  --name tct \
  -p 7681:7681 \
  -v ~/.tct:/data \
  -e TCT_BASIC_AUTH_USER=admin \
  -e TCT_BASIC_AUTH_PASSWORD=change-me-strong-password \
  ghcr.io/waschmittel/tct:latest
```

Basic Auth sends the password with every request in clear text: only use it behind TLS (reverse proxy) or on a private network. The password is visible in `docker inspect` and the process list of the host — prefer `--env-file` or Docker secrets.

---

## Option 2: Docker Compose with OAuth (`oauth2-proxy`)

For internet-facing deployments with OAuth (GitHub, Google, or any OIDC provider like Pocket ID, Authelia, Authentik, or Keycloak), use the example [`docker-compose.yml`](docker-compose.yml) in this directory.

### 1. Configure `.env`

```sh
cd docs/docker
cp .env.example .env
```

Edit `.env` and configure:
- `TCT_HOST_DATA_DIR`: Path to your synced `tct` directory on the host (e.g., `/home/you/.tct`).
- `OAUTH2_PROXY_CLIENT_ID` & `OAUTH2_PROXY_CLIENT_SECRET`: From your OAuth provider (with callback URL `https://<your-domain>/oauth2/callback`).
- `OAUTH2_PROXY_COOKIE_SECRET`: Generate with:
  ```sh
  openssl rand -base64 32 | tr -- '+/' '-_'
  ```
- `OAUTH2_PROXY_GITHUB_USERS`: Restrict login to your username (or configure `OAUTH2_PROXY_EMAIL_DOMAINS` / OIDC claims).
- `OAUTH2_PROXY_EMAIL_DOMAINS`: Required, no default. `*` admits every account of the provider unless the user list or an OIDC issuer narrows it.
- `TCT_UID` / `TCT_GID`: The owner of `TCT_HOST_DATA_DIR` (`id -u` / `id -g`); the container runs unprivileged.

### 2. Start the Stack

```sh
docker compose up -d
```

---

## PWA Installation & Auth Path Exclusions

Once authenticated, you can install `tct` as a standalone app:
- **Desktop (Chrome / Edge / Brave)**: Click the **Install `tct`** button in the address bar.
- **Mobile (Android Chrome / iOS Safari)**: Use **Install app** or **Add to Home Screen**.

### Why Static PWA Metadata Paths Are Excluded from Auth

Both the built-in Nginx Basic Auth config ([`docker/nginx.conf`](../../docker/nginx.conf)) and the [`docker-compose.yml`](docker-compose.yml) `oauth2-proxy` config exclude four static files from authentication:

- `/manifest.webmanifest`
- `/sw.js`
- `/tct-pwa.js`
- `/icon.svg`

These files contain only static build-time constants (no board or user data) and must remain accessible without cookies/headers because mobile OS installers (Android WebAPK builder, iOS home-screen icon fetcher) and browser background Service Worker update checks fetch them outside the authenticated tab context. Meanwhile, `/`, `/token`, and `/ws` remain strictly behind authentication.

---

## Building the Image Locally

From the repository root:

```sh
docker build -f docker/Dockerfile -t tct .
```
