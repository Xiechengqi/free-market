#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

TARGET="${1:-amd64}"
case "$TARGET" in
  amd64) RUST_TARGET="x86_64-unknown-linux-musl" ;;
  arm64) RUST_TARGET="aarch64-unknown-linux-musl" ;;
  *)
    echo "Usage: $0 [amd64|arm64]" >&2
    exit 1
    ;;
esac

VERSION="$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/' || true)"
COMMIT="$(git rev-parse --short=7 HEAD 2>/dev/null || echo unknown)"
COMMIT_MESSAGE="$(git log -1 --pretty=%s 2>/dev/null || echo unknown)"
BUILD_TIME="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
export FREEMARKET_BUILD_SHA="$COMMIT"
export FREEMARKET_COMMIT_MESSAGE="$COMMIT_MESSAGE"
export FREEMARKET_BUILD_TIME="$BUILD_TIME"

echo "[build-ci.sh] target=$TARGET rust_target=$RUST_TARGET version=$VERSION commit=$COMMIT"

if [ -f web-admin/pnpm-lock.yaml ]; then
  pnpm -C web-admin install --frozen-lockfile
else
  pnpm -C web-admin install
fi

pnpm -C web-admin build
touch src/view/admin_spa.rs

if command -v cargo-zigbuild >/dev/null 2>&1; then
  cargo zigbuild --release --target "$RUST_TARGET"
else
  cargo build --release --target "$RUST_TARGET"
fi

echo "[build-ci.sh] built target/${RUST_TARGET}/release/free-market"
