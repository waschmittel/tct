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
