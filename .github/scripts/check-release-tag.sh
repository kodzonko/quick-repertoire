#!/usr/bin/env bash
# Fails unless the release tag is exactly "v<version>" for the [package] version in Cargo.toml.
# Usage: check-release-tag.sh <tag>   (run from the repository root)
set -euo pipefail

tag="${1:?usage: check-release-tag.sh <tag>}"
version="$(awk -F'"' '{ sub(/\r$/, "") } /^\[/ { in_package = ($0 == "[package]") } in_package && /^version *=/ { print $2; exit }' Cargo.toml)"

if [[ -z "${version}" ]]; then
  echo "::error::Could not read the [package] version from Cargo.toml." >&2
  exit 1
fi

if [[ "${tag}" != "v${version}" ]]; then
  echo "::error::Release tag '${tag}' does not match Cargo.toml version ${version} (expected 'v${version}')." >&2
  exit 1
fi

echo "Release tag ${tag} matches Cargo.toml version ${version}."
