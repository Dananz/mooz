#!/usr/bin/env bash
#
# Propagate the repo-root VERSION into the Xcode project, then regenerate it.
#
# VERSION is the single source of truth. This writes it into project.yml's
# MARKETING_VERSION (the only place the app's version literal lives) and runs
# xcodegen. The website needs no sync step - next.config.ts reads VERSION
# directly at build time.
#
# Usage:
#   scripts/sync-version.sh            # sync from the current VERSION file
#   scripts/sync-version.sh 1.2.0      # set VERSION to 1.2.0, then sync
set -euo pipefail
cd "$(dirname "$0")/.."

if [ "${1:-}" != "" ]; then
  printf '%s\n' "$1" > VERSION
fi

VERSION="$(tr -d '[:space:]' < VERSION)"
[ -n "$VERSION" ] || { echo "error: VERSION file is empty" >&2; exit 1; }

# Replace only the MARKETING_VERSION build setting (not the $(MARKETING_VERSION)
# reference in the Info.plist properties block).
/usr/bin/sed -i '' -E "s/(MARKETING_VERSION: )\"[^\"]*\"/\1\"${VERSION}\"/" project.yml
echo "project.yml MARKETING_VERSION -> ${VERSION}"

if command -v xcodegen >/dev/null 2>&1; then
  xcodegen generate >/dev/null
  echo "regenerated Mooz.xcodeproj"
else
  echo "warning: xcodegen not on PATH; run 'xcodegen generate' yourself" >&2
fi
