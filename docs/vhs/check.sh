#!/usr/bin/env bash
# Visual regression check: re-record the vhs tapes and diff each
# final-frame text golden against the committed one.
#
# Note: a golden's column count depends on vhs font metrics, so compare
# only against goldens generated on the same platform/font setup.
set -euo pipefail
cd "$(dirname "$0")/../.."

cargo build -q

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"; rm -f ./tct' EXIT

fail=0
for tape in demo linux-tty; do
    golden="docs/vhs/golden/$tape.txt"
    if [[ -f "$golden" ]]; then
        cp "$golden" "$tmp/expected-$tape.txt"
    fi

    vhs "docs/vhs/$tape.tape" -q

    # vhs dumps every sampled frame into the .txt (blocks separated by ─
    # lines), and frame sampling races against timing-sensitive UI state
    # (e.g. the status-message expiry), so intermediate frames differ
    # between runs. Keep only the final frame — that's what the golden is.
    awk '
        /^─+$/ { if (block != "") last = block; block = ""; next }
        { block = block $0 "\n" }
        END { printf "%s", last }
    ' "$golden" > "$tmp/final-frame.txt"
    mv "$tmp/final-frame.txt" "$golden"

    if [[ ! -f "$tmp/expected-$tape.txt" ]]; then
        echo "$tape: golden created — review and commit $golden"
        continue
    fi

    if diff -u "$tmp/expected-$tape.txt" "$golden"; then
        echo "$tape: vhs golden matches"
    else
        echo "$tape: vhs golden CHANGED — review the diff (and docs/vhs/demo.gif), then commit if intended" >&2
        fail=1
    fi
done

exit $fail
