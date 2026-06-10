#!/usr/bin/env bash
#
# Cut a Mooz release: build -> sign (incl. Sparkle deep re-sign) -> notarize ->
# staple -> appcast -> GitHub release. Version flows from one source (VERSION),
# so the app, the appcast, the git tag and the website all carry the same number.
#
# Usage:
#   scripts/release.sh            # release the current VERSION
#   scripts/release.sh 1.1.0      # bump VERSION to 1.1.0, then release
#
# Prerequisites (one-time, see README "Releasing"):
#   - "Developer ID Application" cert in the login keychain
#   - notarytool keychain profile (default: mooz-notary)
#   - Sparkle EdDSA private key in the login keychain (generate_keys)
#   - gh CLI authenticated for the Dananz/mooz repo
#
# Override via env: MOOZ_SIGN_IDENTITY, MOOZ_NOTARY_PROFILE, MOOZ_REPO.
set -euo pipefail
cd "$(dirname "$0")/.."

REPO_SLUG="${MOOZ_REPO:-Dananz/mooz}"
NOTARY_PROFILE="${MOOZ_NOTARY_PROFILE:-mooz-notary}"
SPARKLE_VERSION="2.9.3"
CACHE="scripts/.cache"

say() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }
die() { echo "error: $1" >&2; exit 1; }

# --- 0. preconditions -------------------------------------------------------
[ -z "$(git status --porcelain)" ] || die "working tree is dirty; commit or stash first"

IDENTITY="${MOOZ_SIGN_IDENTITY:-$(security find-identity -v -p codesigning \
  | sed -nE 's/.*"(Developer ID Application: .*)".*/\1/p' | head -1)}"
[ -n "$IDENTITY" ] || die "no 'Developer ID Application' identity in keychain"

command -v xcodegen >/dev/null || die "xcodegen not found"
command -v gh >/dev/null || die "gh CLI not found"

# --- 1. version bump + sync -------------------------------------------------
say "Versioning"
[ "${1:-}" != "" ] && printf '%s\n' "$1" > VERSION
VERSION="$(tr -d '[:space:]' < VERSION)"
[ -n "$VERSION" ] || die "VERSION is empty"
BUILD_NUM="$(git rev-list --count HEAD)"

# MARKETING_VERSION <- VERSION ; CURRENT_PROJECT_VERSION <- monotonic build number.
/usr/bin/sed -i '' -E "s/(MARKETING_VERSION: )\"[^\"]*\"/\1\"${VERSION}\"/" project.yml
/usr/bin/sed -i '' -E "s/(CURRENT_PROJECT_VERSION: )\"[^\"]*\"/\1\"${BUILD_NUM}\"/" project.yml
xcodegen generate >/dev/null
scripts/check-version.sh "v$VERSION"
echo "releasing v$VERSION (build $BUILD_NUM)"

# --- 2. build ---------------------------------------------------------------
say "Building Release"
DD="$HOME/Library/Developer/Xcode/DerivedData/Mooz-build-release"
xcodebuild -project Mooz.xcodeproj -scheme Mooz -configuration Release \
  -derivedDataPath "$DD" clean build >/dev/null
APP="$DD/Build/Products/Release/Mooz.app"
[ -d "$APP" ] || die "build produced no app"

# --- 3. deep re-sign (Sparkle ships its helpers ad-hoc; notarization needs    -
#        Developer ID + secure timestamp + hardened runtime on every Mach-O) --
say "Code signing (deep)"
FW="$APP/Contents/Frameworks/Sparkle.framework"
if [ -d "$FW" ]; then
  while IFS= read -r item; do
    codesign --force --options runtime --timestamp --preserve-metadata=entitlements \
      -s "$IDENTITY" "$item"
  done < <(find "$FW" \( -name "*.xpc" -o -name "*.app" \) -print \
            | awk '{print length, $0}' | sort -rn | cut -d' ' -f2-)
  [ -f "$FW/Versions/B/Autoupdate" ] && \
    codesign --force --options runtime --timestamp -s "$IDENTITY" "$FW/Versions/B/Autoupdate"
  codesign --force --options runtime --timestamp -s "$IDENTITY" "$FW"
fi
codesign --force --options runtime --timestamp \
  --entitlements Mooz/Mooz.entitlements -s "$IDENTITY" "$APP"
codesign --verify --deep --strict "$APP" || die "deep signature verification failed"

# --- 4. package DMG ---------------------------------------------------------
say "Building DMG"
# dmgbuild lives in a throwaway venv; the styling assets are in scripts/dmg/.
VENV="$CACHE/venv"
[ -x "$VENV/bin/dmgbuild" ] || { python3 -m venv "$VENV"; "$VENV/bin/pip" -q install dmgbuild; }
# Constant filename; the per-tag GitHub release URL (.../v<VERSION>/Mooz.dmg)
# keeps each version's download distinct, so no version is needed in the name.
DMG="build/Mooz.dmg"
mkdir -p build; rm -f "$DMG"
APP="$APP" ICNS="$APP/Contents/Resources/AppIcon.icns" BG="scripts/dmg/background.png" \
  "$VENV/bin/dmgbuild" -s scripts/dmg/settings.py "Mooz" "$DMG" >/dev/null
codesign --force --sign "$IDENTITY" --timestamp "$DMG"

# --- 5. notarize + staple ---------------------------------------------------
say "Notarizing (this can take a few minutes)"
xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait
xcrun stapler staple "$DMG"
spctl -a -t open --context context:primary-signature -vv "$DMG" 2>&1 | grep -i accepted \
  || die "Gatekeeper did not accept the stapled DMG"

# --- 6. appcast (Sparkle signs each enclosure with the keychain EdDSA key) ---
say "Generating appcast"
[ -x "$CACHE/sparkle/bin/generate_appcast" ] || {
  mkdir -p "$CACHE"
  curl -sL "https://github.com/sparkle-project/Sparkle/releases/download/${SPARKLE_VERSION}/Sparkle-${SPARKLE_VERSION}.tar.xz" \
    -o "$CACHE/sparkle.tar.xz"
  mkdir -p "$CACHE/sparkle" && tar -xJf "$CACHE/sparkle.tar.xz" -C "$CACHE/sparkle"
}
STAGE="$(mktemp -d)"; cp "$DMG" "$STAGE/"
mkdir -p web/public
"$CACHE/sparkle/bin/generate_appcast" \
  --download-url-prefix "https://github.com/${REPO_SLUG}/releases/download/v${VERSION}/" \
  -o web/public/appcast.xml "$STAGE"
rm -rf "$STAGE"

# --- 7. commit, tag, push, publish ------------------------------------------
say "Publishing"
git add VERSION project.yml web/public/appcast.xml
git commit -m "release: v${VERSION}"
git tag "v${VERSION}"
git push origin HEAD --tags
gh release create "v${VERSION}" "$DMG" \
  --repo "$REPO_SLUG" --title "Mooz v${VERSION}" --generate-notes

say "Done: v${VERSION} released"
echo "  DMG:     $DMG (notarized + stapled)"
echo "  Appcast: web/public/appcast.xml -> deploys to Pages on push"
echo "  Site:    rebuilds with v${VERSION} automatically"
