import SwiftUI

@main
struct MoozApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @AppStorage("showMenuBarIcon") private var showMenuBarIcon = true

    var body: some Scene {
        Settings {
            SettingsView()
        }

        MenuBarExtra("Mooz", systemImage: "plus.magnifyingglass", isInserted: $showMenuBarIcon) {
            MenuBarView()
        }
        .menuBarExtraStyle(.window)
    }
}
