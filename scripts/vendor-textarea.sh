#!/usr/bin/env bash
# Maintain the vendored fork of tui-textarea-2 (see vendor/README.md).
#
#   scripts/vendor-textarea.sh check            vendor/ == pristine upstream + patch? any newer upstream?
#   scripts/vendor-textarea.sh refresh-patch    regenerate the patch from the current vendor/ edits
#   scripts/vendor-textarea.sh update [VERSION] re-vendor VERSION (default: latest) and re-apply the patch
#
# Needs network (crates.io) for check/refresh-patch/update.
set -euo pipefail

CRATE=tui-textarea-2
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR="$ROOT/vendor/$CRATE"
PATCH="$ROOT/vendor/patches/$CRATE.patch"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

vendored_version() {
  sed -n 's/^version = "\(.*\)"/\1/p' "$VENDOR/Cargo.toml" | head -1
}

latest_version() {
  curl -fsSL -A "tct-vendor-script" "https://crates.io/api/v1/crates/$CRATE" 2>/dev/null |
    sed -n 's/.*"max_stable_version":"\([^"]*\)".*/\1/p'
}

# fetch_pristine VERSION DEST: unpack the untouched upstream crate into DEST.
fetch_pristine() {
  mkdir -p "$2"
  cached="$(find "$HOME/.cargo/registry/cache" -name "$CRATE-$1.crate" 2>/dev/null | head -1)"
  if [ -n "$cached" ] && [ -f "$cached" ]; then
    tar -xzf "$cached" -C "$2" --strip-components=1
  else
    curl -fsSL -A "tct-vendor-script" "https://crates.io/api/v1/crates/$CRATE/$1/download" |
      tar -xz -C "$2" --strip-components=1
  fi
  rm -f "$2/.cargo-ok"
}

# Compare directories ignoring cargo's bookkeeping file.
make_diff() { (cd "$TMP" && diff -ruN --exclude=.cargo-ok a b || true); }

case "${1:-}" in
  check)
    v="$(vendored_version)"
    fetch_pristine "$v" "$TMP/a"
    cp -R "$TMP/a" "$TMP/b"
    (cd "$TMP/b" && patch -p1 -s <"$PATCH")
    rm -rf "$TMP/c" && mkdir "$TMP/c" && cp -R "$VENDOR/." "$TMP/c" && rm -f "$TMP/c/.cargo-ok"
    if ! diff -ruN "$TMP/b" "$TMP/c" >/dev/null; then
      echo "FAIL: vendor/$CRATE differs from upstream $v + patch." >&2
      echo "      Run: scripts/vendor-textarea.sh refresh-patch" >&2
      exit 1
    fi
    echo "OK: vendor/$CRATE == upstream $v + vendor/patches/$CRATE.patch"
    latest="$(latest_version || true)"
    if [ -n "$latest" ] && [ "$latest" != "$v" ]; then
      echo "NOTE: upstream has $latest (vendored $v). Run: scripts/vendor-textarea.sh update"
    fi
    ;;
  refresh-patch)
    v="$(vendored_version)"
    fetch_pristine "$v" "$TMP/a"
    mkdir "$TMP/b" && cp -R "$VENDOR/." "$TMP/b"
    mkdir -p "$(dirname "$PATCH")"
    make_diff >"$PATCH"
    echo "Wrote $PATCH ($(grep -c '^+++ ' "$PATCH") files changed vs upstream $v)"
    ;;
  update)
    new="${2:-$(latest_version)}"
    old="$(vendored_version)"
    echo "Re-vendoring $CRATE: $old -> $new"
    fetch_pristine "$new" "$TMP/new"
    if ! (cd "$TMP/new" && patch -p1 --no-backup-if-mismatch <"$PATCH"); then
      echo "Patch did not apply cleanly to $new; fix the *.rej files in:" >&2
      echo "  $TMP/new  (kept; copy to vendor/$CRATE once fixed, then refresh-patch)" >&2
      trap - EXIT
      exit 1
    fi
    find "$TMP/new" -name '*.orig' -delete
    rm -rf "$VENDOR"
    mkdir -p "$VENDOR"
    cp -R "$TMP/new/." "$VENDOR"
    echo "Vendored $new with patch applied. Now run: scripts/vendor-textarea.sh refresh-patch && cargo test"
    ;;
  *)
    sed -n '2,8p' "$0"
    exit 2
    ;;
esac
