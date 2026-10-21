import Carbon.HIToolbox
import CoreGraphics
import Foundation

/// One non-modifier part of a shortcut: a keyboard key (virtual key code) or a
/// mouse button (CGEvent button number; 2 = middle, 3 = back, 4 = forward).
enum ShortcutInput: Hashable, Sendable {
    case key(UInt16)
    case button(Int)
}

/// The hold-to-zoom shortcut: any modifier combo plus any number of keys and
/// mouse buttons, e.g. ⌃⇧, ⌥ + Z, or ⇧ + Middle Click. Persisted as a compact
/// string ("m131072,k6,b2") so `@AppStorage` can bind it directly.
struct Shortcut: Hashable, Sendable {
    var modifierBits: UInt64
    var inputs: Set<ShortcutInput>

    static let `default` = Shortcut(modifiers: .maskShift)
    static let empty = Shortcut(modifiers: [])

    /// The modifier bits we recognize, in display order, with glyph and name.
    static let orderedModifiers: [(flag: CGEventFlags, glyph: String, name: String)] = [
        (.maskSecondaryFn, "fn", "Fn"),
        (.maskControl, "⌃", "Control"),
        (.maskAlternate, "⌥", "Option"),
        (.maskShift, "⇧", "Shift"),
        (.maskCommand, "⌘", "Command"),
    ]
    static let modifierMask: CGEventFlags = [.maskSecondaryFn, .maskControl, .maskAlternate, .maskShift, .maskCommand]

    init(modifiers: CGEventFlags, inputs: Set<ShortcutInput> = []) {
        self.modifierBits = modifiers.intersection(Self.modifierMask).rawValue
        self.inputs = inputs
    }

    var modifiers: CGEventFlags {
        get { CGEventFlags(rawValue: modifierBits) }
        set { modifierBits = newValue.intersection(Self.modifierMask).rawValue }
    }

    var isEmpty: Bool { modifierBits == 0 && inputs.isEmpty }

    /// True while every part is held. Extra modifiers/keys are allowed (subset
    /// match) so brushing another key mid-gesture doesn't drop it. An empty
    /// shortcut never matches.
    func isSatisfied(flags: CGEventFlags, held: Set<ShortcutInput>) -> Bool {
        !isEmpty && flags.contains(modifiers) && inputs.isSubset(of: held)
    }

    // MARK: - Labels

    /// Keys first (by code), then buttons, so labels are stable.
    var sortedInputs: [ShortcutInput] {
        inputs.sorted { a, b in
            switch (a, b) {
            case let (.key(x), .key(y)): return x < y
            case let (.button(x), .button(y)): return x < y
            case (.key, .button): return true
            case (.button, .key): return false
            }
        }
    }

    /// Glyph + name per modifier, then each key/button name, e.g.
    /// `["⌃ Control", "⇧ Shift", "Z", "Middle Click"]`.
    var components: [String] {
        Self.orderedModifiers.filter { modifiers.contains($0.flag) }.map { "\($0.glyph) \($0.name)" }
            + sortedInputs.map(Self.name(of:))
    }

    /// Compact form for the menu bar, e.g. "⌃⇧Z" or "⇧ + Middle Click".
    var compactLabel: String {
        var head = Self.orderedModifiers.filter { modifiers.contains($0.flag) }.map(\.glyph).joined()
        var tail: [String] = []
        for input in sortedInputs {
            if case .key = input { head += Self.name(of: input) } else { tail.append(Self.name(of: input)) }
        }
        return ([head].filter { !$0.isEmpty } + tail).joined(separator: " + ")
    }

    static func name(of input: ShortcutInput) -> String {
        switch input {
        case .button(let b):
            switch b {
            case 0: return "Left Click"
            case 1: return "Right Click"
            case 2: return "Middle Click"
            case 3: return "Back Button"
            case 4: return "Forward Button"
            default: return "Mouse Button \(b + 1)"
            }
        case .key(let code):
            if let special = specialKeyNames[Int(code)] { return special }
            let typed = typedCharacter(for: code) ?? "Key \(code)"
            return keypadCodes.contains(Int(code)) ? "Num \(typed)" : typed
        }
    }

    private static let specialKeyNames: [Int: String] = [
        kVK_Return: "Return", kVK_Tab: "Tab", kVK_Space: "Space", kVK_Delete: "Delete",
        kVK_Escape: "Esc", kVK_ForwardDelete: "Forward Delete", kVK_Help: "Help",
        kVK_Home: "Home", kVK_End: "End", kVK_PageUp: "Page Up", kVK_PageDown: "Page Down",
        kVK_LeftArrow: "←", kVK_RightArrow: "→", kVK_UpArrow: "↑", kVK_DownArrow: "↓",
        kVK_ANSI_KeypadEnter: "Enter", kVK_ANSI_KeypadClear: "Clear",
        kVK_F1: "F1", kVK_F2: "F2", kVK_F3: "F3", kVK_F4: "F4", kVK_F5: "F5",
        kVK_F6: "F6", kVK_F7: "F7", kVK_F8: "F8", kVK_F9: "F9", kVK_F10: "F10",
        kVK_F11: "F11", kVK_F12: "F12", kVK_F13: "F13", kVK_F14: "F14", kVK_F15: "F15",
        kVK_F16: "F16", kVK_F17: "F17", kVK_F18: "F18", kVK_F19: "F19", kVK_F20: "F20",
    ]

    private static let keypadCodes: Set<Int> = [
        kVK_ANSI_KeypadDecimal, kVK_ANSI_KeypadMultiply, kVK_ANSI_KeypadPlus,
        kVK_ANSI_KeypadDivide, kVK_ANSI_KeypadMinus, kVK_ANSI_KeypadEquals,
        kVK_ANSI_Keypad0, kVK_ANSI_Keypad1, kVK_ANSI_Keypad2, kVK_ANSI_Keypad3, kVK_ANSI_Keypad4,
        kVK_ANSI_Keypad5, kVK_ANSI_Keypad6, kVK_ANSI_Keypad7, kVK_ANSI_Keypad8, kVK_ANSI_Keypad9,
    ]

    /// The character the key types in the current keyboard layout, uppercased
    /// ("Z", "ש", "/"), so the label matches the keycap.
    private static func typedCharacter(for code: UInt16) -> String? {
        guard let source = TISCopyCurrentKeyboardLayoutInputSource()?.takeRetainedValue(),
              let raw = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) else { return nil }
        let layoutData = Unmanaged<CFData>.fromOpaque(raw).takeUnretainedValue() as Data
        var deadKeyState: UInt32 = 0
        var chars = [UniChar](repeating: 0, count: 4)
        var length = 0
        let status = layoutData.withUnsafeBytes { buffer -> OSStatus in
            guard let layout = buffer.baseAddress?.assumingMemoryBound(to: UCKeyboardLayout.self) else { return -1 }
            return UCKeyTranslate(
                layout, code, UInt16(kUCKeyActionDisplay), 0, UInt32(LMGetKbdType()),
                OptionBits(1 << kUCKeyTranslateNoDeadKeysBit), &deadKeyState, chars.count, &length, &chars
            )
        }
        guard status == noErr, length > 0 else { return nil }
        let text = String(utf16CodeUnits: chars, count: length).trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty ? nil : text.uppercased()
    }
}

extension Shortcut: RawRepresentable {
    init?(rawValue: String) {
        var shortcut = Shortcut.empty
        for token in rawValue.split(separator: ",") {
            guard let kind = token.first, let value = Int64(token.dropFirst()) else { return nil }
            switch kind {
            case "m": shortcut.modifiers = CGEventFlags(rawValue: UInt64(bitPattern: value))
            case "k": guard let code = UInt16(exactly: value) else { return nil }; shortcut.inputs.insert(.key(code))
            case "b": guard let button = Int(exactly: value) else { return nil }; shortcut.inputs.insert(.button(button))
            default: return nil
            }
        }
        self = shortcut
    }

    var rawValue: String {
        (["m\(modifierBits)"] + sortedInputs.map { input in
            switch input {
            case .key(let code): return "k\(code)"
            case .button(let b): return "b\(b)"
            }
        }).joined(separator: ",")
    }
}

/// UserDefaults home of the shortcut ("shortcut"). Migrates the older
/// modifier-only "modifierFlags" / "modifierKey" values on first read.
enum ShortcutStore {
    static let key = "shortcut"

    static func current() -> Shortcut {
        let defaults = UserDefaults.standard
        if let raw = defaults.string(forKey: key), let shortcut = Shortcut(rawValue: raw) {
            return shortcut
        }
        let migrated = Shortcut(modifiers: ModifierStore.legacyFlags())
        defaults.set(migrated.rawValue, forKey: key)
        return migrated
    }
}
