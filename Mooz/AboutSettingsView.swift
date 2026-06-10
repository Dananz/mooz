import AppKit
import ServiceManagement
import SwiftUI

/// "About" preferences tab: an app header, the accessibility-permission status
/// (with the same setup banner when not granted), startup options, and version.
struct AboutSettingsView: View {
    @AppStorage("showMenuBarIcon") private var showMenuBarIcon = true
    @AppStorage("launchAtLogin") private var launchAtLogin = false

    @State private var manager = ZoomManager.shared
    @EnvironmentObject private var updater: Updater

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            header

            if !manager.accessibilityGranted {
                permissionBanner
            }

            accessibilityCard
            startupSection
            softwareUpdateSection
        }
        .padding(SettingsMetrics.pagePadding)
        .frame(width: SettingsMetrics.contentWidth)
        .onAppear { manager.checkAccessibility() }
    }

    // MARK: - Header

    private var header: some View {
        HStack(spacing: 14) {
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .interpolation(.high)
                .frame(width: 60, height: 60)

            VStack(alignment: .leading, spacing: 3) {
                Text("Mooz")
                    .font(.title2)
                    .fontWeight(.semibold)
                Text("Trackpad pinch-to-zoom for any mouse.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                Text("Version \(appVersion)")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 6)
    }

    // MARK: - Permission banner

    private var permissionBanner: some View {
        VStack(alignment: .leading, spacing: 10) {
            Label("Accessibility Permission Required", systemImage: "exclamationmark.triangle.fill")
                .font(.headline)
                .foregroundStyle(.orange)

            Text("Mooz cannot intercept mouse events without Accessibility permission. The zoom gesture will not work until this is granted.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            VStack(alignment: .leading, spacing: 4) {
                Text("1. Click \"Open System Settings\" below")
                Text("2. Find Mooz in the list and toggle it on")
                Text("3. Relaunch Mooz")
            }
            .font(.subheadline)
            .foregroundStyle(.secondary)

            Button {
                manager.openAccessibilitySettings()
            } label: {
                Text("Open System Settings → Accessibility")
                    .fontWeight(.medium)
                    .foregroundStyle(.white)
                    .padding(.vertical, 6)
                    .padding(.horizontal, 12)
                    .background(.orange.gradient, in: RoundedRectangle(cornerRadius: 6))
            }
            .buttonStyle(.plain)
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.orange.opacity(0.12), in: RoundedRectangle(cornerRadius: SettingsMetrics.cardCornerRadius))
        .overlay(
            RoundedRectangle(cornerRadius: SettingsMetrics.cardCornerRadius)
                .strokeBorder(.orange.opacity(0.4), lineWidth: 1)
        )
    }

    // MARK: - Accessibility status

    private var accessibilityCard: some View {
        SettingsCard {
            SettingsRow("Accessibility") {
                if manager.accessibilityGranted {
                    Label("Granted", systemImage: "checkmark.circle.fill")
                        .foregroundStyle(.green)
                        .labelStyle(.titleAndIcon)
                } else {
                    Button("Open System Settings") {
                        manager.openAccessibilitySettings()
                    }
                }
            }
        }
    }

    // MARK: - Startup

    private var startupSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            SettingsSectionHeader(title: "Startup")
            SettingsCard {
                SettingsRow("Show menu bar icon") {
                    Toggle("", isOn: $showMenuBarIcon)
                        .toggleStyle(.switch)
                        .labelsHidden()
                }

                CardDivider()

                SettingsRow("Launch at login") {
                    Toggle("", isOn: $launchAtLogin)
                        .toggleStyle(.switch)
                        .labelsHidden()
                        .onChange(of: launchAtLogin) {
                            do {
                                if launchAtLogin {
                                    try SMAppService.mainApp.register()
                                } else {
                                    try SMAppService.mainApp.unregister()
                                }
                            } catch {
                                launchAtLogin = !launchAtLogin
                            }
                        }
                }
            }
        }
    }

    // MARK: - Software Update

    private var softwareUpdateSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            SettingsSectionHeader(title: "Software Update")
            SettingsCard {
                SettingsRow("Automatically check for updates") {
                    Toggle("", isOn: Binding(
                        get: { updater.automaticallyChecksForUpdates },
                        set: { updater.automaticallyChecksForUpdates = $0 }
                    ))
                    .toggleStyle(.switch)
                    .labelsHidden()
                }

                CardDivider()

                SettingsRow("Current version \(appVersion)") {
                    Button("Check Now") {
                        updater.checkForUpdates()
                    }
                    .disabled(!updater.canCheckForUpdates)
                }
            }
        }
    }

    // MARK: - Helpers

    private var appVersion: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0"
    }
}
