import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// "Application" preferences tab. Shows the allow/block list styled like Mos:
/// a list of rounded, selectable app rows (blue when selected), an empty state
/// with a prominent "Add Application" button, and a bottom bar with +/- controls
/// and the allowlist-mode toggle.
///
/// Behavior is unchanged from the old Form: the same `appList`/`listMode`
/// `@AppStorage` keys back the interceptor, and the same add/remove helpers run
/// (including the guard that blocks adding Mooz itself).
struct ApplicationSettingsView: View {
    @AppStorage("listMode") private var listMode = "blocklist"
    @AppStorage("appList") private var appListData = Data()

    @State private var appList: [AppEntry] = []
    @State private var selection: AppEntry.ID?
    @State private var showModeHelp = false

    var body: some View {
        VStack(spacing: 0) {
            if appList.isEmpty {
                emptyState
            } else {
                listView
            }

            Divider()
            bottomBar
        }
        .frame(width: SettingsMetrics.contentWidth, height: 440)
        .onAppear(perform: loadAppList)
    }

    // MARK: - List

    private var listView: some View {
        ScrollView {
            LazyVStack(spacing: 2) {
                ForEach(appList) { app in
                    appRow(app)
                }
            }
            .padding(8)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func appRow(_ app: AppEntry) -> some View {
        let isSelected = selection == app.id
        return Button {
            selection = isSelected ? nil : app.id
        } label: {
            HStack(spacing: 10) {
                appIcon(for: app.bundleId)
                    .frame(width: 26, height: 26)
                VStack(alignment: .leading, spacing: 1) {
                    Text(app.appName)
                        .fontWeight(.medium)
                        .foregroundStyle(isSelected ? Color.white : Color.primary)
                    Text(app.bundleId)
                        .font(.caption)
                        .foregroundStyle(isSelected ? Color.white.opacity(0.8) : Color.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(RoundedRectangle(cornerRadius: 8))
            .background(
                isSelected ? Color.accentColor : Color.clear,
                in: RoundedRectangle(cornerRadius: 8)
            )
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }

    @ViewBuilder
    private func appIcon(for bundleId: String) -> some View {
        if let icon = iconForBundleId(bundleId) {
            Image(nsImage: icon)
                .resizable()
                .interpolation(.high)
        } else {
            Image(systemName: "app.dashed")
                .resizable()
                .scaledToFit()
                .foregroundStyle(.secondary)
        }
    }

    // MARK: - Empty state

    private var emptyState: some View {
        VStack(spacing: 0) {
            Spacer()

            Image(systemName: "square.on.square.dashed")
                .font(.system(size: 50, weight: .ultraLight))
                .foregroundStyle(.tertiary)
                .padding(.bottom, 18)

            Text(emptyTitle)
                .font(.title3)
                .fontWeight(.semibold)

            Text(emptySubtitle)
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 300)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 3)

            addMenu { bigAddLabel }
                .fixedSize()
                .padding(.top, 20)

            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.horizontal, 24)
    }

    private var bigAddLabel: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Add Application")
                    .font(.body)
                    .fontWeight(.semibold)
                Text("Pick a running app or choose from Finder")
                    .font(.caption)
                    .opacity(0.85)
            }
            Spacer(minLength: 14)
            Image(systemName: "magnifyingglass")
                .font(.system(size: 15, weight: .semibold))
        }
        .foregroundStyle(.white)
        .padding(.horizontal, 16)
        .padding(.vertical, 11)
        .frame(width: 290)
        .background(Color.accentColor.gradient, in: RoundedRectangle(cornerRadius: 10))
    }

    // MARK: - Bottom bar

    private var bottomBar: some View {
        HStack(spacing: 10) {
            addRemoveControl
            Spacer()
            allowlistControl
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .frame(height: 44)
    }

    private var addRemoveControl: some View {
        HStack(spacing: 0) {
            addMenu {
                Image(systemName: "plus")
                    .frame(width: 32, height: 22)
                    .contentShape(Rectangle())
            }
            .fixedSize()

            Divider().frame(height: 16)

            Button(action: removeSelected) {
                Image(systemName: "minus")
                    .frame(width: 32, height: 22)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(selection == nil)
        }
        .font(.system(size: 13, weight: .semibold))
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 6))
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .strokeBorder(Color.primary.opacity(0.12), lineWidth: 1)
        )
    }

    private var allowlistControl: some View {
        HStack(spacing: 5) {
            Toggle("Allowlist Mode", isOn: allowlistBinding)
                .toggleStyle(.switch)
                .controlSize(.small)

            Button {
                showModeHelp.toggle()
            } label: {
                Image(systemName: "questionmark.circle")
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .help("What does Allowlist Mode do?")
            .popover(isPresented: $showModeHelp, arrowEdge: .bottom) {
                modeHelpPopover
            }
        }
    }

    private var modeHelpPopover: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("List Mode")
                .font(.headline)
            Label {
                Text("**Blocklist** (off): Mooz works everywhere except the apps in this list.")
            } icon: {
                Image(systemName: "nosign").foregroundStyle(.secondary)
            }
            Label {
                Text("**Allowlist** (on): Mooz works only in the apps in this list.")
            } icon: {
                Image(systemName: "checkmark.circle").foregroundStyle(.green)
            }
        }
        .font(.callout)
        .padding(16)
        .frame(width: 290)
    }

    // MARK: - Add menu (shared by the big button and the "+" button)

    /// A menu with a "Running Applications" submenu (regular running apps, each
    /// with its real icon) and a "Manually Select From Finder…" item.
    private func addMenu<MenuLabel: View>(@ViewBuilder label: () -> MenuLabel) -> some View {
        Menu {
            let running = runningRegularApps()
            Menu("Running Applications") {
                if running.isEmpty {
                    Text("No Running Applications")
                } else {
                    ForEach(running) { app in
                        Button {
                            addEntry(AppEntry(bundleId: app.bundleId, appName: app.name))
                        } label: {
                            Label {
                                Text(app.name)
                            } icon: {
                                Image(nsImage: app.icon)
                            }
                        }
                    }
                }
            }
            Divider()
            Button("Manually Select From Finder…", action: chooseApplication)
        } label: {
            label()
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
    }

    // MARK: - Copy

    private var emptyTitle: String {
        listMode == "allowlist" ? "Mooz is off everywhere" : "Mooz runs everywhere"
    }

    private var emptySubtitle: String {
        listMode == "allowlist"
            ? "Add the apps where you want zoom enabled. It stays off everywhere else."
            : "Add apps to turn zoom off for them. It stays on everywhere else."
    }

    private var allowlistBinding: Binding<Bool> {
        Binding(
            get: { listMode == "allowlist" },
            set: { listMode = $0 ? "allowlist" : "blocklist" }
        )
    }

    // MARK: - Running applications

    private struct RunningApp: Identifiable {
        let bundleId: String
        let name: String
        let icon: NSImage
        var id: String { bundleId }
    }

    /// Currently-running regular (Dock-visible) apps, excluding Mooz itself
    /// and any app already in the list, sorted by localized name.
    private func runningRegularApps() -> [RunningApp] {
        let selfId = Bundle.main.bundleIdentifier
        let existing = Set(appList.map(\.bundleId))
        var seen = Set<String>()
        var result: [RunningApp] = []

        for app in NSWorkspace.shared.runningApplications where app.activationPolicy == .regular {
            guard let bundleId = app.bundleIdentifier,
                  bundleId != selfId,
                  !existing.contains(bundleId),
                  seen.insert(bundleId).inserted
            else { continue }

            let baseIcon = app.icon ?? NSWorkspace.shared.icon(for: .application)
            let icon = (baseIcon.copy() as? NSImage) ?? baseIcon
            icon.size = NSSize(width: 16, height: 16)
            result.append(RunningApp(bundleId: bundleId, name: app.localizedName ?? bundleId, icon: icon))
        }

        return result.sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }

    // MARK: - List persistence & mutation
    //
    // These helpers are preserved from the original SettingsView. The self-block
    // guard in `addEntry` (never add Mooz itself) is intentional — keep it.

    private func loadAppList() {
        if let list = try? JSONDecoder().decode([AppEntry].self, from: appListData) {
            appList = list
        }
    }

    private func saveAppList() {
        if let data = try? JSONEncoder().encode(appList) {
            appListData = data
        }
    }

    /// Single add path: never add Mooz itself, never add duplicates.
    private func addEntry(_ entry: AppEntry) {
        guard entry.bundleId != Bundle.main.bundleIdentifier else { return }
        guard !appList.contains(where: { $0.bundleId == entry.bundleId }) else { return }
        appList.append(entry)
        saveAppList()
    }

    private func addCurrentApp() {
        guard let entry = BlocklistManager.currentFrontmostApp() else { return }
        addEntry(entry)
    }

    private func chooseApplication() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.application]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.directoryURL = URL(fileURLWithPath: "/Applications")
        panel.prompt = "Choose"
        guard panel.runModal() == .OK, let url = panel.url,
              let bundleId = Bundle(url: url)?.bundleIdentifier else { return }
        let name = FileManager.default.displayName(atPath: url.path)
            .replacingOccurrences(of: ".app", with: "")
        addEntry(AppEntry(bundleId: bundleId, appName: name))
    }

    private func removeSelected() {
        guard let id = selection, let app = appList.first(where: { $0.id == id }) else { return }
        removeApp(app)
        selection = nil
    }

    private func removeApp(_ app: AppEntry) {
        appList.removeAll { $0.bundleId == app.bundleId }
        saveAppList()
    }

    private func iconForBundleId(_ bundleId: String) -> NSImage? {
        guard let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleId) else { return nil }
        return NSWorkspace.shared.icon(forFile: url.path)
    }
}
