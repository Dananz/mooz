import AppKit

class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return false
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Single-instance enforcement
        let dominated = NSRunningApplication.runningApplications(
            withBundleIdentifier: Bundle.main.bundleIdentifier ?? ""
        )
        let others = dominated.filter { $0 != NSRunningApplication.current }
        if let existing = others.first {
            existing.activate()
            DispatchQueue.main.async { NSApp.terminate(nil) }
            return
        }

        let manager = ZoomManager.shared
        // Only prompt for Accessibility if not already granted.
        // requestAccessibility uses AXIsProcessTrustedWithOptions which shows
        // a system dialog — skip it if permission is already in place.
        manager.checkAccessibility()
        if !manager.accessibilityGranted {
            manager.requestAccessibility()
        }

        if !manager.accessibilityGranted {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                NSApp.sendAction(Selector(("showPreferencesWindow:")), to: nil, from: nil)
                NSApp.activate(ignoringOtherApps: true)
            }
        } else {
            // Open settings on first launch only when permission is already granted
            let hasLaunchedBefore = UserDefaults.standard.bool(forKey: "hasLaunchedBefore")
            if !hasLaunchedBefore {
                UserDefaults.standard.set(true, forKey: "hasLaunchedBefore")
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                    NSApp.sendAction(Selector(("showPreferencesWindow:")), to: nil, from: nil)
                    NSApp.activate(ignoringOtherApps: true)
                }
            }
        }

        // Start zoom interception
        manager.startIfEnabled()
    }
}
