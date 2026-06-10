import Combine
import Sparkle
import SwiftUI

/// SwiftUI-facing wrapper around Sparkle's standard updater.
///
/// `SPUStandardUpdaterController` owns the scheduled background checks, the
/// update download/install flow, and Sparkle's stock UI. This object starts it
/// on init and republishes the two bits of state the UI needs: whether a manual
/// check is allowed right now, and whether automatic checks are enabled.
@MainActor
final class Updater: ObservableObject {
    private let controller: SPUStandardUpdaterController

    /// Mirrors `SPUUpdater.canCheckForUpdates` so the menu item / button can
    /// disable itself while a check is already running.
    @Published private(set) var canCheckForUpdates = false

    /// Two-way mirror of the user's "check automatically" preference. The setter
    /// writes through to Sparkle (which persists it in user defaults).
    @Published var automaticallyChecksForUpdates: Bool {
        didSet { controller.updater.automaticallyChecksForUpdates = automaticallyChecksForUpdates }
    }

    init() {
        controller = SPUStandardUpdaterController(
            startingUpdater: true,
            updaterDelegate: nil,
            userDriverDelegate: nil,
        )
        // Initialize the wrapper without triggering didSet (no write-back during init).
        _automaticallyChecksForUpdates = Published(
            initialValue: controller.updater.automaticallyChecksForUpdates,
        )
        controller.updater.publisher(for: \.canCheckForUpdates)
            .assign(to: &$canCheckForUpdates)
    }

    /// Manual "Check for Updates…" action. Shows Sparkle's UI (no update found,
    /// update available, error) on the main thread.
    func checkForUpdates() {
        controller.checkForUpdates(nil)
    }
}
