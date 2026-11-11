#!/usr/bin/env bash
# Seed the data shown in the website screenshots. Needs TCT_DATA_DIR (fresh dir)
# and TCT (the tct binary). Run via site/showcase/build.sh.
set -euo pipefail
TCT=${TCT:-./target/debug/tct}
DIR=$(cd "$(dirname "$0")" && pwd)
B="Release 2.0"
CARD="Release 2.0 — planning overview"

$TCT boards --create "$B"
for l in Backlog Doing Review Done; do $TCT lists "$B" --create "$l"; done
for l in feature bug docs; do $TCT labels "$B" --create "$l"; done

$TCT cards "$B" --create Doing "$CARD"
$TCT cards "$B" --edit "$CARD" --description "$(cat "$DIR/description.md")" --due 2027-06-30
for lab in feature docs; do $TCT labels "$B" --assign "$CARD" "$lab"; done
for item in "Freeze the feature list" "Write the migration guide" "Run the load test" "Tag the release"; do
    $TCT checklist "$B" "$CARD" --add "$item"
done
$TCT checklist "$B" "$CARD" --toggle 1

card() { # list title label [due]
    $TCT cards "$B" --create "$1" "$2"
    [ -z "${3:-}" ] || $TCT labels "$B" --assign "$2" "$3"
}
card Doing "Fix flaky login test" bug
card Doing "Speed up start-up"
card Review "Cache board listing" feature
card Review "Document env variables" docs
card Backlog "Dark mode for the web UI" feature
card Backlog "Crash on empty checklist" bug
card Backlog "Translate the user guide" docs
card Backlog "Plugin API sketch"
card Done "Atomic writes" feature
card Done "Release notes draft" docs
$TCT cards "$B" --edit "Fix flaky login test" --description "Fails once in ten runs on CI."
$TCT checklist "$B" "Fix flaky login test" --add "Reproduce locally"
$TCT checklist "$B" "Fix flaky login test" --add "Add retry-free fix"
$TCT checklist "$B" "Fix flaky login test" --toggle 1

for b in Personal Ops; do $TCT boards --create "$b"; done
$TCT lists Personal --create Today
$TCT cards Personal --create Today "Book dentist"
