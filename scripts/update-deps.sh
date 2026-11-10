#!/usr/bin/env bash
# Update the Rust toolchain pin and all crate dependencies, then verify.
#
#   scripts/update-deps.sh
#
# Needs network. Not covered (manual): GitHub Actions SHAs in .github/workflows,
# the Docker base images, and the vendored tui-textarea-2 (reported only;
# update with scripts/vendor-textarea.sh update).
set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> Toolchain"
rustup update stable
version="$(rustc +stable --version | awk '{print $2}')"
sed -i.bak -E "s/^channel = .*/channel = \"$version\"/" rust-toolchain.toml
rm rust-toolchain.toml.bak
echo "pinned $version"

echo "==> Crates (semver-compatible)"
cargo update

echo "==> Crates (breaking)"
if cargo upgrade --version >/dev/null 2>&1; then
  cargo upgrade --incompatible
  cargo update
else
  echo "cargo-edit missing (cargo install cargo-edit): breaking bumps skipped" >&2
fi

echo "==> Vendored fork"
scripts/vendor-textarea.sh check || true

echo "==> Verify"
cargo fmt --check || cargo fmt   # new rustfmt may restyle; review the diff
RUSTFLAGS="-D warnings" cargo build
cargo test -- --test-threads=1
node --test docker/pwa/tct-pwa.test.js

echo "Done. Review: git diff Cargo.toml Cargo.lock rust-toolchain.toml"
