#!/usr/bin/env bash
# Regenerate the website screenshots and showcase PDFs (needs vhs, pdftoppm, cwebp).
# Outputs: site/img/*.webp, site/export/showcase-*.pdf. Run from the repo root.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q
# Run from a neutral cwd so the PDF dialog's folder list shows no private paths.
BIN=/tmp/tct-site-bin
HOME_DIR=/tmp/tct-site-home
export TCT_DATA_DIR=/tmp/tct-site-demo TCT="$BIN/tct" PATH="$BIN:$PATH"
rm -rf "$TCT_DATA_DIR" "$BIN" "$HOME_DIR"
mkdir -p "$TCT_DATA_DIR" "$BIN" site/img site/export "$HOME_DIR"/{Desktop,Documents,Downloads,Projects/{tct,website,notes}}
cp target/debug/tct "$BIN/tct"
site/showcase/seed.sh >/dev/null

for style in mono color; do
    tct cards "Release 2.0" --export "planning overview" --style "$style" \
        --out "site/export/showcase-$style.pdf" >/dev/null
    rm -f site/img/pdf-"$style"*.png
    pdftoppm -png -r 100 "site/export/showcase-$style.pdf" "site/img/pdf-$style"
done
vhs site/showcase/shots.tape
# Lossless WebP: much smaller than PNG for text and flat colour.
for f in site/img/*.png; do cwebp -quiet -lossless -z 9 "$f" -o "${f%.png}.webp" && rm "$f"; done
ls -la site/img
