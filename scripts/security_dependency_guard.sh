#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

echo "[guard] checking for reachable MySQL/RSA deps from bctn-backend"
if cargo tree -p bctn-backend | rg -q "sqlx-mysql|sqlx-postgres|rsa"; then
  echo "[guard] FAIL: unexpected reachable db/transitive crypto dependency detected"
  cargo tree -p bctn-backend | rg "sqlx-mysql|sqlx-postgres|rsa" || true
  exit 1
fi

echo "[guard] running cargo audit with repo policy"
cargo audit -q

echo "[guard] running rust 1.92 compatibility checks"
cargo +1.92.0 check -p bctn-backend
cargo +1.92.0 check -p bctn-ingest

echo "[guard] running workspace package checks"
cargo check -p bctn-backend
cargo test -p bctn-backend
cargo check -p bctn-ingest
cargo test -p bctn-ingest

echo "[guard] OK"
