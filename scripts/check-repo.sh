#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "${script_dir}/.." && pwd)"

if command -v python3 >/dev/null 2>&1; then
  python3 "${repo_root}/scripts/check-repo.py" "$@"
elif command -v python >/dev/null 2>&1; then
  python "${repo_root}/scripts/check-repo.py" "$@"
else
  echo "Python 3 is required to run repository baseline checks." >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "Cargo from the pinned Rust toolchain is required to run Rust checks." >&2
  exit 1
fi

cd "${repo_root}"
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
