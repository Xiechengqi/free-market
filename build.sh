#!/usr/bin/env bash

set -euo pipefail -x
cd "$(dirname "$0")"

# 1. Build the admin SPA (Vue + Vite + NaiveUI) → web-admin/dist/
if [ ! -d web-admin/node_modules ]; then
  echo "[build.sh] installing web-admin npm deps (first run)"
  pnpm -C web-admin install --no-frozen-lockfile
fi
echo "[build.sh] building admin SPA"
pnpm -C web-admin build

# 2. rust-embed embeds web-admin/dist/ at compile time. Cargo's change tracking
#    doesn't follow files inside that folder, so we force a rebuild of the
#    admin_spa module whenever build.sh runs. This is a no-op when the
#    embedded bytes haven't actually changed, but guarantees freshness.
touch src/view/admin_spa.rs

# 3. Build the Rust binary.
echo "[build.sh] building Rust binary"
cargo build "$@"
