import SwiftUI

@main
struct MoozApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @AppStorage("showMenuBarIcon") private var showMenuBarIcon = true
    @StateObject private var updater = Updater()

    var body: some Scene {
        Settings {
            SettingsView()
                .environmentObject(updater)
        }

        MenuBarExtra("Mooz", systemImage: "plus.magnifyingglass", isInserted: $showMenuBarIcon) {
            MenuBarView()
                .environmentObject(updater)
        }
        .menuBarExtraStyle(.window)
    }
}
