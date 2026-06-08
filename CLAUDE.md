# Mooz

macOS menu bar utility that emulates native trackpad pinch-zoom for any mouse.

## Tech Stack
- Swift 6.0 (strict concurrency: complete), SwiftUI, macOS 14+
- XcodeGen — `project.yml` is the source of truth; `xcodegen generate` makes
  `Mooz.xcodeproj` (not committed). Run `xcodegen generate` after pulling.
- CGEventTap (Quartz) for input interception
- Full IOHID gesture synthesis (original `MoozGesture.m`) for native magnify
- No package-manager dependencies and no vendored third-party code

## Architecture
Flat file structure with an @Observable @MainActor orchestrator.

- `MoozApp.swift` — @main, MenuBarExtra, Settings scene
- `AppDelegate.swift` — single-instance enforcement, Accessibility prompt, lifecycle
- `ZoomManager.swift` — @Observable @MainActor orchestrator (singleton); reads
  settings live from UserDefaults and drives the interceptor
- `GestureInterceptor.swift` — CGEventTap: intercept modifier+drag/scroll, map to
  magnification, pin the cursor (warp + suppression interval), watchdog
- `MagnificationEmitter.swift` — posts the magnify gesture via `MoozEmitMagnify`
- `ModifierStore.swift` — modifier combo persisted as CGEventFlags + glyph/name helpers
- `BlocklistManager.swift` — per-app blocklist/allowlist via bundle ID
- Settings (tabbed, glass): `SettingsView` (tab container), `GeneralSettingsView`,
  `ApplicationSettingsView`, `AboutSettingsView`, `SettingsComponents` (cards,
  tab bar, `CapsuleSegmentedControl`, glass window), `ModifierRecorderField`
- `MenuBarView.swift` — menu bar popover (on/off, open settings)
- `MoozGesture.{h,m}` — original Objective-C synthesizer of the IOHID gesture
  blob (struct layout in `MoozHIDLayout.h`), exposed to Swift via
  `Mooz-Bridging-Header.h`

## Build & Run
`xcodegen generate`, open `Mooz.xcodeproj`, run (Cmd+R). Signed with a stable
local identity (`CODE_SIGN_IDENTITY` in `project.yml`) so the TCC Accessibility
grant survives rebuilds. Requires Accessibility permission (System Settings ▸
Privacy & Security ▸ Accessibility).
