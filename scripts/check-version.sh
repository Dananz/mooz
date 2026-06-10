#!/usr/bin/env bash
#
# Version drift guard. Fails if the version is not identical across every source:
#   - VERSION                        (repo-root source of truth)
#   - project.yml MARKETING_VERSION  (the macOS app)
#   - the release git tag            (only when building a tagged release: vX.Y.Z)
#
# The website is not checked because next.config.ts reads VERSION directly at
# build time and therefore cannot drift.
#
# Usage:
#   scripts/check-version.sh           # check VERSION vs the app
#   scripts/check-version.sh v1.2.0    # also require the tag to match
# In CI it picks the tag up from GITHUB_REF automatically.
set -euo pipefail
cd "$(dirname "$0")/.."

fail() { echo "VERSION DRIFT: $1" >&2; exit 1; }

VERSION="$(tr -d '[:space:]' < VERSION)"
[ -n "$VERSION" ] || fail "VERSION file is empty"

APP_VER="$(grep -E '^[[:space:]]*MARKETING_VERSION:' project.yml | head -1 | sed -E 's/.*"(.*)".*/\1/')"
[ -n "$APP_VER" ] || fail "MARKETING_VERSION not found in project.yml"
[ "$APP_VER" = "$VERSION" ] || \
  fail "VERSION=$VERSION but project.yml MARKETING_VERSION=$APP_VER (run scripts/sync-version.sh)"

TAG="${1:-}"
if [ -z "$TAG" ] && [[ "${GITHUB_REF:-}" == refs/tags/* ]]; then
  TAG="${GITHUB_REF#refs/tags/}"
fi
if [ -n "$TAG" ]; then
  [ "$TAG" = "v$VERSION" ] || fail "git tag $TAG does not match v$VERSION"
fi

echo "version consistent: VERSION=$VERSION, app=$APP_VER${TAG:+, tag=$TAG}"
