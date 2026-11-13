#!/usr/bin/env bash
# Preview the website locally: build into <outdir> (default: public) and serve
# it at http://localhost:<port> (default 8000). Needs node and `npm ci` in
# site/guide; offers to install `mmdr` (required) and `fswatch` (optional,
# rebuilds on change). Set MMDR if mmdr is not on PATH.
set -euo pipefail
cd "$(dirname "$0")/.."
out=${1:-public}
port=${2:-8000}

# ask <question> <install command…>: runs the command only after an explicit "y".
ask() {
  local q=$1; shift
  [[ -t 0 ]] || return 1
  read -r -p "$q [y/N] " a
  [[ $a == [yY]* ]] && "$@"
}

[[ -d site/guide/node_modules ]] || ask "Install guide dependencies (npm ci)?" npm ci --prefix site/guide || true
if ! command -v "${MMDR:-mmdr}" >/dev/null; then
  ask "mmdr (mermaid-rs-renderer) not found. Install with cargo?" \
    cargo install mermaid-rs-renderer --version 0.3.1 --locked ||
    { echo "mmdr is required for the guide's diagrams." >&2; exit 1; }
fi
command -v fswatch >/dev/null || ask "fswatch not found (rebuild on change). Install with brew?" brew install fswatch || true

site/build.sh "$out"
if command -v fswatch >/dev/null; then
  (fswatch -o -l 1 site docs/user-guide.md docs/export-examples docs/vhs/demo.gif |
    while read -r _; do site/build.sh "$out" || true; done) &
  trap 'kill 0' EXIT
fi
echo "Serving $out at http://localhost:$port"
python3 -I -m http.server "$port" --directory "$out" --bind 127.0.0.1
