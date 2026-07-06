#!/usr/bin/env bash
# Visual regression check: re-record docs/vhs/demo.tape and diff the
# final-frame text golden against the committed one.
#
# Note: the golden's column count depends on vhs font metrics, so compare
# only against goldens generated on the same platform/font setup.
set -euo pipefail
cd "$(dirname "$0")/../.."

cargo build -q

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cp docs/vhs/golden/demo.txt "$tmp/expected.txt"

vhs docs/vhs/demo.tape -q
rm -f ./tct

# vhs dumps every sampled frame into the .txt (blocks separated by ─ lines),
# and frame sampling races against timing-sensitive UI state (e.g. the 3s
# status-message expiry), so intermediate frames differ between runs.
# Keep only the final frame — that's what the golden is.
awk '
    /^─+$/ { if (block != "") last = block; block = ""; next }
    { block = block $0 "\n" }
    END { printf "%s", last }
' docs/vhs/golden/demo.txt > "$tmp/final-frame.txt"
mv "$tmp/final-frame.txt" docs/vhs/golden/demo.txt

if diff -u "$tmp/expected.txt" docs/vhs/golden/demo.txt; then
    echo "vhs golden matches"
else
    echo "vhs golden CHANGED — review the diff (and docs/vhs/demo.gif), then commit if intended" >&2
    exit 1
fi
