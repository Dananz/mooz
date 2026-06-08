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

## License

[MIT](LICENSE) © Tomer Danan. See [NOTICE](NOTICE) for third-party attributions.
