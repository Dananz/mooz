import AppKit
import SwiftUI

// MARK: - Metrics

/// Shared layout constants for the Settings window. The width is fixed across
/// every tab; each tab supplies its own height so the native preferences window
/// animates to fit the selected tab (see `SettingsView`).
enum SettingsMetrics {
    static let contentWidth: CGFloat = 460
    static let pagePadding: CGFloat = 20
    static let cardCornerRadius: CGFloat = 10
    static let rowHPadding: CGFloat = 14
    static let rowVPadding: CGFloat = 9
}

// MARK: - Card

/// A grouped "card" matching the modern macOS System Settings look on a glass
/// window: a translucent, frosted panel that reads as a layer floating on the
/// window vibrancy (rather than an opaque grey box), with a gentle hairline
/// border. Rows are laid out vertically; separate them with `CardDivider`.
struct SettingsCard<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        VStack(spacing: 0) {
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            .thinMaterial,
            in: RoundedRectangle(cornerRadius: SettingsMetrics.cardCornerRadius)
        )
        .overlay(
            RoundedRectangle(cornerRadius: SettingsMetrics.cardCornerRadius)
                .strokeBorder(Color.primary.opacity(0.07), lineWidth: 1)
        )
    }
}

/// A hairline divider inset to align with card row content.
struct CardDivider: View {
    var body: some View {
        Divider()
            .padding(.leading, SettingsMetrics.rowHPadding)
    }
}

// MARK: - Section header

/// Small, uppercase, secondary label shown above a `SettingsCard`, mirroring the
/// group headers in macOS System Settings.
struct SettingsSectionHeader: View {
    let title: String

    var body: some View {
        Text(title)
            .font(.caption)
            .fontWeight(.semibold)
            .textCase(.uppercase)
            .foregroundStyle(.secondary)
            .padding(.horizontal, 6)
    }
}

// MARK: - Row

/// A standard settings row: a leading title (with an optional subtitle) and a
/// trailing control. Use for simple toggle/label rows; build custom HStacks for
/// rows with sliders or pickers.
struct SettingsRow<Trailing: View>: View {
    let title: String
    var subtitle: String?
    @ViewBuilder var trailing: Trailing

    init(_ title: String, subtitle: String? = nil, @ViewBuilder trailing: () -> Trailing) {
        self.title = title
        self.subtitle = subtitle
        self.trailing = trailing()
    }

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                if let subtitle {
                    Text(subtitle)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer(minLength: 12)
            trailing
        }
        .padding(.horizontal, SettingsMetrics.rowHPadding)
        .padding(.vertical, SettingsMetrics.rowVPadding)
    }
}

// MARK: - Settings window chrome (glass + constant title)

/// Backs the Settings window with the two things the native preferences scene
/// doesn't provide:
/// 1. Continuous glass — a behind-window `NSVisualEffectView` spanning the whole
///    window (transparent titlebar + `.fullSizeContentView`), so the toolbar band
///    reads as the same glass with the traffic lights + tabs floating on top.
/// 2. A constant window title "Settings" (a KVO guard re-asserts it whenever the
///    native TabView tries to set it to the selected tab's name).
///
/// The per-tab height is left to SwiftUI's native preferences `TabView` (it snaps
/// without animation; an animated resize was tried several ways and each had
/// artifacts, so we keep the native behavior). It does NOT set
/// `isOpaque=false`/`.clear` (that made the whole window transparent +
/// click-through); the effect view is the surface.
struct WindowChrome: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = ChromeEffectView()
        view.material = .underWindowBackground
        view.blendingMode = .behindWindow
        view.state = .active
        return view
    }

    func updateNSView(_ nsView: NSVisualEffectView, context: Context) {}
}

private final class ChromeEffectView: NSVisualEffectView {
    private var titleObserver: NSKeyValueObservation?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard let window else { return }
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .visible
        window.titlebarSeparatorStyle = .none
        window.styleMask.insert(.fullSizeContentView)
        window.title = "Settings"
        titleObserver = window.observe(\.title, options: [.new]) { win, _ in
            if win.title != "Settings" { win.title = "Settings" }
        }
    }
}

// MARK: - Capsule segmented control

/// A compact capsule-style selector for a small set of string choices. The
/// selected segment is filled with the accent color and the highlight slides
/// between segments via `matchedGeometryEffect`. Drop-in for a 2–3 option
/// `Picker(.segmented)` where a more bespoke look is wanted.
struct CapsuleSegmentedControl: View {
    struct Option: Identifiable, Hashable {
        let value: String
        let label: String
        var id: String { value }
    }

    let options: [Option]
    @Binding var selection: String
    @Namespace private var ns

    var body: some View {
        HStack(spacing: 2) {
            ForEach(options) { option in
                let isSelected = option.value == selection
                Text(option.label)
                    .font(.system(size: 12, weight: .medium))
                    .lineLimit(1)
                    .padding(.vertical, 5)
                    .padding(.horizontal, 14)
                    .foregroundStyle(isSelected ? Color.white : Color.secondary)
                    .background {
                        if isSelected {
                            Capsule(style: .continuous)
                                .fill(Color.accentColor)
                                .matchedGeometryEffect(id: "capsuleHighlight", in: ns)
                        }
                    }
                    .contentShape(Capsule(style: .continuous))
                    .onTapGesture {
                        withAnimation(.snappy(duration: 0.22)) { selection = option.value }
                    }
                    .accessibilityAddTraits(isSelected ? [.isSelected, .isButton] : .isButton)
                    .accessibilityLabel(option.label)
            }
        }
        .padding(3)
        .background(Color.primary.opacity(0.08), in: Capsule(style: .continuous))
    }
}
