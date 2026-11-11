#!/usr/bin/env bash
# Assemble the website into <outdir> (default: public): landing page, user
# guide (rendered from docs/user-guide.md), icon, demo and PDF samples.
# Needs node and `npm ci` run in site/guide once. Run from anywhere.
set -euo pipefail
cd "$(dirname "$0")/.."
out=${1:-public}
rm -rf "$out"
mkdir -p "$out/export"
cp site/index.html site/style.css docs/vhs/demo.gif docker/pwa/icon.svg "$out/"
cp -r site/img "$out/"
cp -r docs/export-examples/*.pdf docs/export-examples/color "$out/export/"
cp site/export/*.pdf "$out/export/"
node site/guide/build.mjs "$out"
