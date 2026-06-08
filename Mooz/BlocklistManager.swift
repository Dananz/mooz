import AppKit

struct AppEntry: Codable, Identifiable, Hashable, Sendable {
    let bundleId: String
    let appName: String

    var id: String { bundleId }
}

final class BlocklistManager: Sendable {
    enum ListMode: String, CaseIterable, Sendable {
        case blocklist, allowlist
    }

    private func loadAppList() -> [AppEntry] {
        guard let data = UserDefaults.standard.data(forKey: "appList"),
              let list = try? JSONDecoder().decode([AppEntry].self, from: data) else {
            return []
        }
        return list
    }

    private func loadListMode() -> ListMode {
        let raw = UserDefaults.standard.string(forKey: "listMode") ?? "blocklist"
        return ListMode(rawValue: raw) ?? .blocklist
    }

    /// Returns true if Mooz should intercept events for the current frontmost app.
    /// Must be called from the main actor since it accesses NSWorkspace.
    @MainActor
    func shouldInterceptCurrentApp() -> Bool {
        guard let bundleId = NSWorkspace.shared.frontmostApplication?.bundleIdentifier else {
            return true
        }
        let appList = loadAppList()
        let isInList = appList.contains { $0.bundleId == bundleId }

        switch loadListMode() {
        case .blocklist:
            return !isInList
        case .allowlist:
            return isInList
        }
    }

    /// Grabs the current frontmost app info for adding to the list.
    /// Must be called from the main actor since it accesses NSWorkspace.
    @MainActor
    static func currentFrontmostApp() -> AppEntry? {
        guard let app = NSWorkspace.shared.frontmostApplication,
              let bundleId = app.bundleIdentifier else { return nil }
        return AppEntry(bundleId: bundleId, appName: app.localizedName ?? bundleId)
    }

    static func addApp(_ entry: AppEntry) {
        var list = loadList()
        guard !list.contains(where: { $0.bundleId == entry.bundleId }) else { return }
        list.append(entry)
        saveList(list)
    }

    static func removeApp(bundleId: String) {
        var list = loadList()
        list.removeAll { $0.bundleId == bundleId }
        saveList(list)
    }

    private static func loadList() -> [AppEntry] {
        guard let data = UserDefaults.standard.data(forKey: "appList"),
              let list = try? JSONDecoder().decode([AppEntry].self, from: data) else {
            return []
        }
        return list
    }

    private static func saveList(_ list: [AppEntry]) {
        if let data = try? JSONEncoder().encode(list) {
            UserDefaults.standard.set(data, forKey: "appList")
        }
    }
}
