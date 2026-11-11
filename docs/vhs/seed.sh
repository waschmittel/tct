#!/usr/bin/env bash
# Seed deterministic demo data for the vhs tapes.
# Expects TCT_DATA_DIR to point at a fresh directory.
set -euo pipefail

TCT=${TCT:-./target/debug/tct}

$TCT boards --create Demo
$TCT lists Demo --create "To Do"
$TCT lists Demo --create "Done"
$TCT cards Demo --create "To Do" "Fix login flow"
$TCT cards Demo --edit "Fix login flow" --description "Token expires too early."
$TCT cards Demo --create "To Do" "Redesign dashboard"
$TCT cards Demo --create "Done" "Ship release notes"
$TCT labels Demo --create bug
$TCT labels Demo --assign "Fix login flow" bug
$TCT checklist Demo "Fix login flow" --add "Reproduce"
$TCT checklist Demo "Fix login flow" --add "Write failing test"
$TCT checklist Demo "Fix login flow" --toggle 1
$TCT labels Demo --create urgent
for l in feature chore docs; do $TCT labels Demo --create "$l"; done

# card LIST TITLE [LABELS (comma-separated)] [DESCRIPTION] [DUE] [CHECKLIST (|-separated)] [DONE COUNT]
card() {
    local list=$1 title=$2 labels=${3:-} desc=${4:-} due=${5:-} items=${6:-} done=${7:-0} l i
    $TCT cards Demo --create "$list" "$title"
    [ -z "$desc" ] || $TCT cards Demo --edit "$title" --description "$desc"
    [ -z "$due" ] || $TCT cards Demo --edit "$title" --due "$due"
    if [ -n "$labels" ]; then
        IFS=, read -ra ls <<<"$labels"
        for l in "${ls[@]}"; do $TCT labels Demo --assign "$title" "$l"; done
    fi
    if [ -n "$items" ]; then
        IFS='|' read -ra its <<<"$items"
        for l in "${its[@]}"; do $TCT checklist Demo "$title" --add "$l"; done
        for ((i = 1; i <= done; i++)); do $TCT checklist Demo "$title" --toggle "$i"; done
    fi
}

# A realistic backlog (long enough that the list scrolls) with varied detail.
card "To Do" "Add dark mode to the settings page" feature "Follow the OS setting, with a manual override." 2027-03-31
card "To Do" "Migrate CI to GitHub Actions" chore "Port the pipeline; the release job runs last." "" \
    "Port the test job|Port the build job|Port the release job" 1
card "To Do" "Fix typo in the onboarding email" bug
card "To Do" "Rate-limit the public API" feature,urgent "100 requests per minute and key; return 429 with Retry-After." 2027-02-15
card "To Do" "Upgrade PostgreSQL to 17" chore "" "" "Test the migration on staging|Schedule the maintenance window"
card "To Do" "Write an on-call runbook" docs "" 2027-04-30
card "To Do" "Cache product images on the CDN" feature "Cache for a week; purge on upload."
card "To Do" "Add CSV export to reports"
card "To Do" "Investigate slow search queries" bug "p95 went from 80 ms to 400 ms after the last release." "" \
    "Reproduce on staging|Check the query plan|Add the missing index" 1
card "To Do" "Replace the deprecated auth library" chore,urgent "Security fixes have stopped upstream." 2027-01-20
card "To Do" "Localize error messages" feature
card "To Do" "Clean up unused feature flags" chore

# Done
$TCT labels Demo --assign "Ship release notes" docs
card "Done" "Set up error tracking" chore "" "" "Create the project|Add the SDK|Alert on new errors" 3
card "Done" "Fix broken password reset link" bug,urgent "The link pointed at the staging host."
card "Done" "Publish the v1.2 changelog" docs
$TCT boards --create Roadmap
$TCT lists Roadmap --create "Q1"
$TCT lists Roadmap --create "Q2"
$TCT cards Roadmap --create "Q1" "Security audit"
$TCT cards Roadmap --create "Q2" "Mobile app"

