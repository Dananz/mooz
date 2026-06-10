import AppKit
import CoreGraphics
import SwiftUI

/// Reference-type recording session. Must be a class (not View @State) because
/// the NSEvent monitor closure escapes; mutating a captured View struct's @State
/// writes to a dead copy and never persists.
@MainActor
final class ModifierRecorderModel: ObservableObject {
    @Published var isRecording = false
    @Published var captured: CGEventFlags = []
    private var monitor: Any?
    private var keyMonitor: Any?
    var onCommit: ((CGEventFlags) -> Void)?

    func toggle() { isRecording ? finalize() : start() }

    func start() {
        isRecording = true
        captured = []
        monitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            MainActor.assumeIsolated {
                guard let self else { return }
                let f = Self.cgFlags(from: event.modifierFlags)
                if f.isEmpty {
                    if !self.captured.isEmpty { self.finalize() }
                } else {
                    self.captured.formUnion(f)
                }
            }
            return event
        }
        // Esc cancels the in-progress recording (and is swallowed so it does not
        // also close the Settings window).
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            var swallow = false
            MainActor.assumeIsolated {
                guard let self else { return }
                if event.keyCode == 53 { // Escape
                    self.cancel()
                    swallow = true
                }
            }
            return swallow ? nil : event
        }
    }

    /// Commit the captured modifiers (when at least one modifier is held).
    func finalize() {
        let masked = captured.intersection([.maskShift, .maskControl, .maskAlternate, .maskCommand])
        if !masked.isEmpty { onCommit?(masked) }
        stop()
    }

    /// Abort recording without committing — keeps the previously stored value.
    func cancel() {
        captured = []
        stop()
    }

    func stop() {
        isRecording = false
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
        keyMonitor = nil
    }

    private static func cgFlags(from ns: NSEvent.ModifierFlags) -> CGEventFlags {
        var f: CGEventFlags = []
        if ns.contains(.shift) { f.insert(.maskShift) }
        if ns.contains(.control) { f.insert(.maskControl) }
        if ns.contains(.option) { f.insert(.maskAlternate) }
        if ns.contains(.command) { f.insert(.maskCommand) }
        return f
    }
}

/// Shows the stored zoom modifier combo as a compact pill. Clicking the pill
/// opens a popover that records a new combo by listening for flagsChanged while
/// the Settings window is key: press the keys, release to capture (Esc or
/// clicking away cancels). The row never changes size — idle or recording — so
/// the card and window stay put; the recording UI lives entirely in the popover.
struct ModifierRecorderField: View {
    @Binding var flagsRaw: Int
    @StateObject private var model = ModifierRecorderModel()
    @State private var isPresentingRecorder = false

    private var currentFlags: CGEventFlags {
        CGEventFlags(rawValue: UInt64(bitPattern: Int64(flagsRaw)))
    }

    var body: some View {
        HStack(spacing: 12) {
            Text("Modifier Keys")
            Spacer(minLength: 12)
            Button { isPresentingRecorder = true } label: {
                ComboPill(flags: currentFlags, isActive: isPresentingRecorder)
            }
            .buttonStyle(.plain)
            .help("Click to record the zoom modifier keys")
            .popover(isPresented: $isPresentingRecorder, arrowEdge: .bottom) {
                ModifierRecorderPopover(model: model)
            }
        }
        .onAppear {
            model.onCommit = { flags in
                flagsRaw = Int(bitPattern: UInt(truncatingIfNeeded: flags.rawValue))
            }
        }
        // Drive the capture session from the popover's presentation: start on
        // open, stop on close. `finalize()`/`cancel()` flip `isRecording` to
        // false (on key release, Esc, or commit), which dismisses the popover.
        .onChange(of: isPresentingRecorder) { _, presenting in
            if presenting { model.start() } else { model.stop() }
        }
        .onChange(of: model.isRecording) { _, recording in
            if !recording { isPresentingRecorder = false }
        }
        .onDisappear { model.stop() }
    }
}

/// Joins a combo as glyph + name per modifier, separated by " + ",
/// e.g. "⌃ Control + ⇧ Shift". Empty when nothing is set.
private func comboLabel(_ flags: CGEventFlags) -> String {
    ModifierStore.components(flags)
        .map { "\($0.glyph) \($0.name)" }
        .joined(separator: " + ")
}

// MARK: - Pill + popover

/// The compact, fixed-size field control: the stored combo as a single rounded
/// pill (joined with " + "). Highlights while its popover is open.
private struct ComboPill: View {
    let flags: CGEventFlags
    var isActive: Bool = false
    @State private var hovering = false

    var body: some View {
        let label = comboLabel(flags)
        Text(label.isEmpty ? "Click to set" : label)
            .font(.system(size: 12, weight: .medium))
            .foregroundStyle(label.isEmpty ? Color.secondary : Color.primary)
            .lineLimit(1)
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(fill, in: Capsule(style: .continuous))
            .overlay(
                Capsule(style: .continuous)
                    .strokeBorder(
                        isActive ? Color.accentColor.opacity(0.6) : Color.primary.opacity(0.08),
                        lineWidth: 1
                    )
            )
            .contentShape(Capsule(style: .continuous))
            .onHover { hovering = $0 }
    }

    private var fill: Color {
        if isActive { return Color.accentColor.opacity(0.16) }
        if hovering { return Color.primary.opacity(0.1) }
        return Color.primary.opacity(0.06)
    }
}

/// The recording UI, shown inside the popover so the row never resizes. Shows a
/// pulsing indicator, a live preview of the currently-held combo (joined with
/// " + "), and a hint. Releasing the keys commits; Esc cancels.
private struct ModifierRecorderPopover: View {
    @ObservedObject var model: ModifierRecorderModel
    @State private var pulse = false

    var body: some View {
        let preview = comboLabel(model.captured)
        VStack(spacing: 12) {
            HStack(spacing: 6) {
                Circle()
                    .fill(Color.red)
                    .frame(width: 7, height: 7)
                    .opacity(pulse ? 1 : 0.3)
                Text("Recording")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(.secondary)
            }

            Text(preview.isEmpty ? "Press your modifier keys…" : preview)
                .font(.system(size: 16, weight: preview.isEmpty ? .regular : .medium))
                .foregroundStyle(preview.isEmpty ? Color.secondary : Color.primary)
                .lineLimit(1)
                .frame(maxWidth: .infinity)

            Text("Release to set, or press Esc to cancel.")
                .font(.caption)
                .foregroundStyle(.tertiary)
                .multilineTextAlignment(.center)
        }
        .padding(16)
        .frame(width: 260)
        .onAppear {
            withAnimation(.easeInOut(duration: 0.6).repeatForever(autoreverses: true)) {
                pulse = true
            }
        }
    }
}
