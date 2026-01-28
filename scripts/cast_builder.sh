#!/usr/bin/env bash
set -euo pipefail

ARGS=()
if [[ $# -gt 0 ]]; then
  ARGS+=("--out" "$1")
fi

cargo run -- cast "${ARGS[@]}"
