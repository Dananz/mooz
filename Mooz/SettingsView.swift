import SwiftUI

/// Root of the Settings scene, on SwiftUI's native macOS preferences `TabView`
/// (toolbar tabs the system pins in the titlebar — they never move). SwiftUI
/// sizes the window to each tab's content, but SNAPS the height with no
/// animation. `WindowChrome` lets that snap happen, then intercepts it and
/// replays it as an eased, frame-only window animation (so the content — already
/// laid out at the new size — stays put and the window simply grows/shrinks to
/// reveal it). It also adds the continuous glass and the constant "Settings"
/// title.
struct SettingsView: View {
    var body: some View {
        TabView {
            GeneralSettingsView()
                .tabItem { Label("General", systemImage: "gearshape") }
            ApplicationSettingsView()
                .tabItem { Label("Application", systemImage: "square.grid.2x2") }
            AboutSettingsView()
                .tabItem { Label("About", systemImage: "info.circle") }
        }
        .frame(width: SettingsMetrics.contentWidth)
        .toolbarBackground(.hidden, for: .windowToolbar)
        .background(WindowChrome().ignoresSafeArea())
    }
}
