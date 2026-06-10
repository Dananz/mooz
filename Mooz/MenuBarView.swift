import SwiftUI

struct MenuBarView: View {
    @AppStorage("modifierFlags") private var modifierFlagsRaw = 131072 // .maskShift
    @AppStorage("inputSource") private var inputSource = "mouseDrag"
    @Environment(\.openSettings) private var openSettings
    @EnvironmentObject private var updater: Updater

    @State private var manager = ZoomManager.shared

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Mooz")
                    .font(.callout)
                    .fontWeight(.semibold)
                Spacer()
                Circle()
                    .fill(manager.accessibilityGranted && manager.isRunning ? Color.green : Color.red)
                    .frame(width: 7, height: 7)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)

            Divider()

            if !manager.accessibilityGranted {
                permissionWarning
            } else {
                activeContent
            }

            Divider()

            // Check for updates
            Button("Check for Updates...") {
                updater.checkForUpdates()
            }
            .buttonStyle(.plain)
            .font(.caption)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .disabled(!updater.canCheckForUpdates)

            Divider()

            // Footer
            HStack {
                Button("Settings...") {
                    openSettings()
                    NSApp.activate(ignoringOtherApps: true)
                }
                .font(.caption)

                Spacer()

                Button("Quit") {
                    NSApp.terminate(nil)
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
        }
        .frame(width: 240)
        .onAppear { manager.checkAccessibility() }
    }

    // MARK: - Active Content

    private var activeContent: some View {
        HStack {
            Image(systemName: "keyboard")
                .foregroundStyle(.secondary)
            Text(modifierLabel)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Spacer()
            Toggle("", isOn: Binding(
                get: { manager.isRunning },
                set: { _ in manager.toggle() }
            ))
            .toggleStyle(.switch)
            .labelsHidden()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
    }

    // MARK: - Permission Warning

    private var permissionWarning: some View {
        VStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.title2)
                .foregroundStyle(.orange)
            Text("Accessibility Permission Required")
                .font(.callout)
                .fontWeight(.medium)
                .multilineTextAlignment(.center)
            Text("Mooz needs Accessibility access to intercept mouse events.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            Button {
                manager.openAccessibilitySettings()
            } label: {
                Text("Open System Settings")
                    .fontWeight(.medium)
                    .foregroundStyle(.white)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 6)
                    .background(.orange.gradient, in: RoundedRectangle(cornerRadius: 6))
            }
            .buttonStyle(.plain)
        }
        .padding(16)
    }

    // MARK: - Helpers

    private var modifierLabel: String {
        let flags = CGEventFlags(rawValue: UInt64(bitPattern: Int64(modifierFlagsRaw)))
        let mod = ModifierStore.symbols(flags)
        let input = inputSource == "scrollWheel" ? "Scroll" : "Drag"
        return "\(mod.isEmpty ? "—" : mod) + \(input)"
    }
}
