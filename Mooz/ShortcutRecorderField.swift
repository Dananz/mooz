import AppKit
import Carbon.HIToolbox
import CoreGraphics
import SwiftUI

/// Reference-type recording session. Must be a class (not View @State) because
/// the NSEvent monitor closures escape; mutating a captured View struct's @State
/// writes to a dead copy and never persists.
@MainActor
final class ShortcutRecorderModel: ObservableObject {
    @Published var isRecording = false
    @Published var captured = Shortcut.empty
    private var heldModifiers: CGEventFlags = []
    private var heldInputs: Set<ShortcutInput> = []
    private var monitors: [Any] = []
    var onCommit: ((Shortcut) -> Void)?

    func start() {
        stop()
        isRecording = true
        captured = .empty
        heldModifiers = []
        heldInputs = []
        // Keys are swallowed while recording so they don't type, beep, or fire
        // menu shortcuts (⌘W would close Settings). Esc cancels.
        let local = NSEvent.addLocalMonitorForEvents(
            matching: [.flagsChanged, .keyDown, .keyUp, .otherMouseDown, .otherMouseUp]
        ) { [weak self] event in
            let swallow = MainActor.assumeIsolated { self?.handle(event) ?? false }
            return swallow ? nil : event
        }
        // Extra mouse buttons pressed while the pointer is outside our windows.
        let global = NSEvent.addGlobalMonitorForEvents(
            matching: [.otherMouseDown, .otherMouseUp]
        ) { [weak self] event in
            MainActor.assumeIsolated { _ = self?.handle(event) }
        }
        monitors = [local, global].compactMap { $0 }
    }

    /// Returns true when the event should be swallowed.
    private func handle(_ event: NSEvent) -> Bool {
        guard isRecording else { return false }
        switch event.type {
        case .flagsChanged:
            heldModifiers = Self.cgFlags(from: event.modifierFlags)
            captured.modifiers.formUnion(heldModifiers)
            commitIfReleased()
            return false
        case .keyDown:
            if event.keyCode == UInt16(kVK_Escape) {
                cancel()
                return true
            }
            if !event.isARepeat {
                heldInputs.insert(.key(event.keyCode))
                captured.inputs.insert(.key(event.keyCode))
            }
            return true
        case .keyUp:
            heldInputs.remove(.key(event.keyCode))
            commitIfReleased()
            return true
        case .otherMouseDown:
            heldInputs.insert(.button(event.buttonNumber))
            captured.inputs.insert(.button(event.buttonNumber))
            return true
        case .otherMouseUp:
            heldInputs.remove(.button(event.buttonNumber))
            commitIfReleased()
            return true
        default:
            return false
        }
    }

    /// Everything let go after at least one press: that's the shortcut.
    private func commitIfReleased() {
        guard heldModifiers.isEmpty, heldInputs.isEmpty, !captured.isEmpty else { return }
        onCommit?(captured)
        stop()
    }

    /// Abort recording without committing — keeps the previously stored value.
    func cancel() {
        captured = .empty
        stop()
    }

    func stop() {
        isRecording = false
        monitors.forEach(NSEvent.removeMonitor)
        monitors = []
    }

    private static func cgFlags(from ns: NSEvent.ModifierFlags) -> CGEventFlags {
        var f: CGEventFlags = []
        if ns.contains(.shift) { f.insert(.maskShift) }
        if ns.contains(.control) { f.insert(.maskControl) }
        if ns.contains(.option) { f.insert(.maskAlternate) }
        if ns.contains(.command) { f.insert(.maskCommand) }
        if ns.contains(.function) { f.insert(.maskSecondaryFn) }
        return f
    }
}

/// Shows the stored zoom shortcut as a compact pill. Clicking the pill opens a
/// popover that records a new shortcut: hold any modifiers, keys, and extra
/// mouse buttons, release to capture (Esc or clicking away cancels). The row never changes size — idle or recording — so
/// the card and window stay put; the recording UI lives entirely in the popover.
struct ShortcutRecorderField: View {
    @Binding var shortcut: Shortcut
    @StateObject private var model = ShortcutRecorderModel()
    @State private var isPresentingRecorder = false

    var body: some View {
        HStack(spacing: 12) {
            Text("Shortcut")
            Spacer(minLength: 12)
            Button { isPresentingRecorder = true } label: {
                ComboPill(shortcut: shortcut, isActive: isPresentingRecorder)
            }
            .buttonStyle(.plain)
            .help("Click to record the zoom shortcut")
            .popover(isPresented: $isPresentingRecorder, arrowEdge: .bottom) {
                ShortcutRecorderPopover(model: model)
            }
        }
        .onAppear {
            model.onCommit = { shortcut = $0 }
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

/// Joins a shortcut's parts with " + ", e.g. "⌃ Control + ⇧ Shift + Middle
/// Click". Empty when nothing is set.
private func comboLabel(_ shortcut: Shortcut) -> String {
    shortcut.components.joined(separator: " + ")
}

// MARK: - Pill + popover

/// The compact, fixed-size field control: the stored combo as a single rounded
/// pill (joined with " + "). Highlights while its popover is open.
private struct ComboPill: View {
    let shortcut: Shortcut
    var isActive: Bool = false
    @State private var hovering = false

    var body: some View {
        let label = comboLabel(shortcut)
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
/// pulsing indicator, a live preview of everything pressed so far (joined with
/// " + "), and a hint. Releasing everything commits; Esc cancels.
private struct ShortcutRecorderPopover: View {
    @ObservedObject var model: ShortcutRecorderModel
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

            Text(preview.isEmpty ? "Hold keys or mouse buttons…" : preview)
                .font(.system(size: 16, weight: preview.isEmpty ? .regular : .medium))
                .foregroundStyle(preview.isEmpty ? Color.secondary : Color.primary)
                .lineLimit(2)
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity)

            Text("Any keys, plus middle, back, or forward mouse buttons. Release to set, or press Esc to cancel.")
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
