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

if diff -u "$tmp/expected.txt" docs/vhs/golden/demo.txt; then
    echo "vhs golden matches"
else
    echo "vhs golden CHANGED — review the diff (and docs/vhs/demo.gif), then commit if intended" >&2
    exit 1
fi
