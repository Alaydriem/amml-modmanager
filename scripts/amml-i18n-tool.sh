#!/usr/bin/env bash
# Runs amml-i18n-tool from the amml source that Cargo.toml selects (the git tag), so the tool version always matches
# the amml-i18n runtime that the crates link.
set -euo pipefail

manifest=$(cargo metadata --format-version 1 --locked \
  | jq -r '.packages[] | select(.name == "amml-i18n-tool") | .manifest_path')
if [ -z "$manifest" ]; then
  echo "amml-i18n-tool is not in the dependency graph" >&2
  exit 1
fi

exec cargo run -q --locked --manifest-path "$manifest" --target-dir target/amml-i18n-tool -- "$@"
