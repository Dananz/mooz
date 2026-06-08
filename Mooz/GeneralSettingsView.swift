import SwiftUI

/// "General" preferences tab: enable switch, the modifier-key recorder, input
/// source, sensitivity, and the cursor-anchor behavior. All values are bound to
/// the same `@AppStorage` keys the interceptor reads live.
struct GeneralSettingsView: View {
    @AppStorage("isEnabled") private var isEnabled = true
    @AppStorage("modifierFlags") private var modifierFlagsRaw = 131072 // .maskShift
    @AppStorage("inputSource") private var inputSource = "mouseDrag"
    @AppStorage("sensitivity") private var sensitivity = 0.5
    @AppStorage("anchorCursor") private var anchorCursor = true

    @State private var manager = ZoomManager.shared

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            enableCard
            gestureSection
            behaviorSection
        }
        .padding(SettingsMetrics.pagePadding)
        .frame(width: SettingsMetrics.contentWidth)
        .onChange(of: isEnabled) {
            manager.startIfEnabled()
        }
    }

    // MARK: - Enable

    private var enableCard: some View {
        SettingsCard {
            SettingsRow(
                "Enable Mooz",
                subtitle: "Emulate trackpad pinch-to-zoom with your mouse."
            ) {
                Toggle("", isOn: $isEnabled)
                    .toggleStyle(.switch)
                    .labelsHidden()
            }
        }
    }

    // MARK: - Gesture

    private var gestureSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            SettingsSectionHeader(title: "Gesture")
            SettingsCard {
                ModifierRecorderField(flagsRaw: $modifierFlagsRaw)
                    .padding(.horizontal, SettingsMetrics.rowHPadding)
                    .padding(.vertical, SettingsMetrics.rowVPadding)

                CardDivider()

                HStack(spacing: 12) {
                    Text("Input Source")
                    Spacer(minLength: 12)
                    CapsuleSegmentedControl(
                        options: [
                            .init(value: "mouseDrag", label: "Mouse Drag"),
                            .init(value: "scrollWheel", label: "Scroll Wheel"),
                        ],
                        selection: $inputSource
                    )
                }
                .padding(.horizontal, SettingsMetrics.rowHPadding)
                .padding(.vertical, SettingsMetrics.rowVPadding)

                CardDivider()

                sensitivityRow
            }
        }
    }

    private var sensitivityRow: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("Sensitivity")
                Spacer()
                Text(sensitivityLabel)
                    .font(.callout)
                    .monospacedDigit()
                    .foregroundStyle(.secondary)
            }
            HStack(spacing: 8) {
                Image(systemName: "tortoise.fill")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                Slider(value: $sensitivity, in: 0.1...3.0, step: 0.1)
                Image(systemName: "hare.fill")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
        }
        .padding(.horizontal, SettingsMetrics.rowHPadding)
        .padding(.vertical, SettingsMetrics.rowVPadding)
    }

    // MARK: - Behavior

    private var behaviorSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            SettingsSectionHeader(title: "Behavior")
            SettingsCard {
                SettingsRow(
                    "Anchor cursor while zooming",
                    subtitle: "Locks the pointer in place during a zoom so the page doesn't pan. Turn off to let the cursor move freely."
                ) {
                    Toggle("", isOn: $anchorCursor)
                        .toggleStyle(.switch)
                        .labelsHidden()
                }
            }
        }
    }

    // MARK: - Helpers

    private var sensitivityLabel: String {
        String(format: "%.1f×", sensitivity)
    }
}
