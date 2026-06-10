# Mooz

**Trackpad pinch-to-zoom for any mouse, on macOS.**

Mooz lets a regular mouse do the thing only a trackpad could: smooth, continuous
pinch-zoom inside any app. Hold a modifier key and move the mouse (or scroll),
and the page zooms — in Safari, Chrome, Firefox, Zen, Preview, Maps, anywhere
that supports the system pinch gesture. The pointer stays anchored in place while
you zoom, so the page doesn't pan out from under you.

> **Why "Mooz"?** It's *zoom* spelled backwards — and it reads a little like
> *mouse*. The whole app is about pointing your mouse at the one trackpad trick
> it never had.

## Features

- **Native pinch-zoom with a mouse** — synthesizes a real macOS magnify gesture,
  so apps zoom exactly like they do from a trackpad (not a `⌘ +/-` shortcut hack).
- **Works everywhere** — including Gecko browsers (Firefox/Zen), which need a
  fully-formed gesture event that most tools don't produce.
- **Cursor anchoring** — the pointer is pinned where the zoom starts so content
  doesn't slide away. Toggle it off if you'd rather the cursor move freely.
- **Configurable trigger** — record any modifier or combo (⇧, ⌘, ⌃, ⌥, or e.g.
  ⇧⌘) and choose **drag** or **scroll** as the input.
- **Adjustable sensitivity** with smoothing for a buttery feel.
- **Per-app control** — a blocklist or allowlist so Mooz only acts where you want.
- **Menu-bar app** — lives in the menu bar (no Dock icon), with a glass,
  tabbed Settings window.

## Requirements

- macOS 14 or later
- **Accessibility permission** (System Settings ▸ Privacy & Security ▸
  Accessibility) — required to intercept input and post the gesture.

## Versioning

The version lives in one place: the `VERSION` file at the repo root. Everything
derives from it, so the app, the website and the release tag can never drift:

- **App**: `project.yml` reads it into `MARKETING_VERSION`. Run
  `scripts/sync-version.sh` after editing `VERSION` (or just use the release
  script, which does it for you).
- **Website**: `web/next.config.ts` reads `VERSION` at build time and shows it
  in the footer, so a rebuild always reflects the current value.
- **Release tag**: `scripts/release.sh` tags the release `v<VERSION>`.

`scripts/check-version.sh` fails if any of these disagree, and it runs in CI
(`.github/workflows/version-guard.yml`) on every PR and tag.

## Releasing

One command builds, signs, notarizes and publishes a release:

```bash
scripts/release.sh 1.1.0   # or no argument to release the current VERSION
```

It bumps `VERSION`, builds Release, deep-signs the app and the embedded Sparkle
helpers with Developer ID, notarizes and staples the DMG, regenerates the
Sparkle appcast (`web/public/appcast.xml`), pushes the tag, and creates the
GitHub release with the DMG attached. Existing users get the update through the
in-app updater; the website footer updates on the next Pages deploy.

One-time setup:

- A **Developer ID Application** certificate in your login keychain.
- A notarytool keychain profile:
  `xcrun notarytool store-credentials mooz-notary --apple-id <id> --team-id Q2V86449AC --password <app-specific-password>`.
- A Sparkle EdDSA signing key (`generate_keys`). **Back up the private key**;
  losing it means existing users can no longer auto-update.
- `gh` authenticated for the repo.

## License

[MIT](LICENSE) © Tomer Danan. See [NOTICE](NOTICE) for third-party attributions.
