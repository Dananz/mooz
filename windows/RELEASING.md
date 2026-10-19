# Windows releases

Mooz uses Velopack 1.2.161 for a per-user EXE installer, Windows Apps uninstall registration, a Start menu shortcut, package verification and in-app updates. No administrator rights or .NET runtime are required to run Mooz. The build tool needs .NET 8.

## Build a test installer

On Windows, with Rust and .NET 8 installed:

```powershell
cd windows
dotnet tool install --global vpk --version 1.2.161
cargo build --release --locked
./scripts/package.ps1
```

`dist/releases/Mooz-win-Setup.exe` is the installer. Keep every generated `.nupkg` and `releases.win.json` together for update delivery. `SHA256SUMS.txt` covers the release artifacts. The package script stages only Mooz.exe and LICENSE. Test installers are unsigned and show Windows publisher/reputation warnings. Never describe them as signed releases.

The `windows` workflow produces these artifacts for each Windows PR and runs `smoke-install.ps1` on a disposable runner. That test installs and launches Mooz, creates a local 99.0.0 fixture (never publish it), checks/downloads/applies it using the Rust SDK, verifies settings and startup persistence, launches the upgraded app, and uninstalls. It does not simulate real mouse input or certify every target app's zoom behavior.

## Configure signing once

Add repository secrets `WINDOWS_SIGNING_PFX_BASE64` (a base64 encoded trusted code-signing PFX) and `WINDOWS_SIGNING_PFX_PASSWORD`. Use GitHub's encrypted secret UI or `gh secret set`, never commit the PFX. Hardware-backed certificates need a different signing provider; adapt the script to Velopack's `--azureTrustedSignFile` or `--signTemplate` for that certificate. The script imports the PFX into the runner's current-user certificate store, timestamps signatures, and deletes the imported certificate and temporary file afterward.

Unsigned artifacts remain available in CI, but the `windows-release` workflow refuses an unsigned public release. A signature does not guarantee SmartScreen reputation for a new publisher.

## Ship a version

1. Update the repository's shared `VERSION` and the macOS version in `project.yml` together, following `scripts/check-version.sh`. Edit `windows/RELEASE-NOTES.md`.
2. Merge the reviewed changes. Push a tag `windows-v<VERSION>` pointing to that exact commit, or manually dispatch `windows-release` on that commit. Do not reuse an existing version.
3. The workflow builds, tests, signs, smoke-tests and creates a **draft GitHub Release** containing the EXE, packages, feed and checksums. Inspect the draft, install it on a real Windows machine, then publish the release. Keep it a stable release (not prerelease) to reach installed stable clients. Publishing is the final delivery step.
4. Existing installations check public stable GitHub Releases after 30 seconds and every 24 hours. Settings > Updates also checks manually. Downloads and restarts always require a click. The check preference disables background checks. Draft and prerelease assets never reach stable clients.

The app's GitHub source is fixed to `Dananz/mooz`; no token is embedded. It scans recent releases for the `win` feed, so Windows and macOS releases can coexist. Retain release feeds and full packages. Future packages must retain pack ID `Mooz`, channel `win`, and executable `Mooz.exe`.

Settings live outside the installation in `%APPDATA%\Mooz\config.json`, so updates preserve them. Start-with-Windows is opt-in. Update callbacks repair an existing startup entry; uninstall removes that entry. The update worker stops and waits for the tray to release its hooks, then tells Update.exe to wait for Settings to exit before replacing binaries and relaunching the tray. Bare copies of Mooz.exe direct users to the installer instead of attempting a self-overwrite.

## If an update fails

The app displays the error and allows retry. It never downgrades automatically, downloads on its own, or silently restarts. Check GitHub connectivity and Windows security software, then download the latest Setup.exe if necessary. Preserve `%APPDATA%\Mooz` when reinstalling to keep preferences. A rollback should be published as a new, higher version containing the known-good code.
