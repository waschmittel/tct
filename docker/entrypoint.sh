#!/bin/sh
set -eu

export TCT_DATA_DIR="${TCT_DATA_DIR:-/data}"
export TERM="${TERM:-xterm-256color}"
export COLORTERM="${COLORTERM:-truecolor}"

mkdir -p "$TCT_DATA_DIR" /etc/nginx/snippets

# Configure opt-in HTTP Basic Auth if TCT_BASIC_AUTH_USER & TCT_BASIC_AUTH_PASSWORD are provided
if [ -n "${TCT_BASIC_AUTH_USER:-}" ] && [ -n "${TCT_BASIC_AUTH_PASSWORD:-}" ]; then
    HASH="$(printf '%s' "$TCT_BASIC_AUTH_PASSWORD" | openssl passwd -5 -stdin)"
    printf '%s:%s\n' "$TCT_BASIC_AUTH_USER" "$HASH" > /etc/nginx/.htpasswd
    chmod 600 /etc/nginx/.htpasswd
    cat > /etc/nginx/snippets/tct-auth.conf <<'EOF'
auth_basic "tct";
auth_basic_user_file /etc/nginx/.htpasswd;
EOF
    echo "tct-web: HTTP Basic Auth enabled for user '${TCT_BASIC_AUTH_USER}'." >&2
else
    cat > /etc/nginx/snippets/tct-auth.conf <<'EOF'
auth_basic off;
EOF
    echo "tct-web: running without built-in Basic Auth. Bind to 127.0.0.1, set TCT_BASIC_AUTH_USER/TCT_BASIC_AUTH_PASSWORD, or place behind an OAuth/reverse proxy." >&2
fi

# Start ttyd on loopback; nginx on :7681 serves PWA assets and proxies /ws + /token.
ttyd \
    --writable \
    --interface 127.0.0.1 \
    --port 7682 \
    -t titleFixed="tct" \
    -t scrollback=0 \
    -t 'theme={"background":"#000000","black":"#000000","foreground":"#e0e0e8","cursor":"#5ec4ff"}' \
    tct ${TCT_ARGS:-} &
TTYD_PID=$!

cleanup() {
    kill -TERM "$TTYD_PID" 2>/dev/null || true
    nginx -s quit 2>/dev/null || true
}
trap cleanup INT TERM

nginx -g 'daemon off;' &
NGINX_PID=$!

# Exit as soon as either process exits
while kill -0 "$TTYD_PID" 2>/dev/null && kill -0 "$NGINX_PID" 2>/dev/null; do
    sleep 1
done

cleanup
exit 1
