import CoreGraphics
import Foundation

/// Persists the zoom modifier combo as a set of CGEventFlags bits in
/// UserDefaults ("modifierFlags"), and renders it as glyphs (⇧⌘). Supports
/// multi-key combos like Shift+Command. Migrates the legacy single-key
/// "modifierKey" string on first read.
enum ModifierStore {
    static let key = "modifierFlags"
    private static let legacyKey = "modifierKey"
    static let defaultFlags: CGEventFlags = .maskShift

    /// The bits we recognize, in display order, with their glyph and name.
    static let orderedBits: [(flag: CGEventFlags, glyph: String, name: String)] = [
        (.maskControl, "⌃", "Control"),
        (.maskAlternate, "⌥", "Option"),
        (.maskShift, "⇧", "Shift"),
        (.maskCommand, "⌘", "Command"),
    ]

    static func currentFlags() -> CGEventFlags {
        let defaults = UserDefaults.standard
        if defaults.object(forKey: key) != nil {
            return CGEventFlags(rawValue: UInt64(bitPattern: Int64(defaults.integer(forKey: key))))
        }
        // Migrate legacy single-key value, if any.
        if let legacy = defaults.string(forKey: legacyKey) {
            let migrated = flags(forLegacy: legacy)
            save(migrated)
            return migrated
        }
        return defaultFlags
    }

    static func save(_ flags: CGEventFlags) {
        let masked = flags.intersection([.maskShift, .maskControl, .maskAlternate, .maskCommand])
        UserDefaults.standard.set(Int(bitPattern: UInt(truncatingIfNeeded: masked.rawValue)), forKey: key)
    }

    /// Human-readable glyphs, e.g. "⇧⌘". Empty string when nothing is set.
    static func symbols(_ flags: CGEventFlags) -> String {
        orderedBits.filter { flags.contains($0.flag) }.map(\.glyph).joined()
    }

    /// One modifier's glyph and name, e.g. glyph "⇧" / name "Shift".
    struct Component: Identifiable, Hashable {
        let glyph: String
        let name: String
        var id: String { name }
    }

    /// Per-modifier glyph + name in display order, e.g.
    /// `[⇧ Shift, ⌘ Command]`. Empty when nothing is set. Used to render
    /// Mos-style pills that show the glyph and its name.
    static func components(_ flags: CGEventFlags) -> [Component] {
        orderedBits
            .filter { flags.contains($0.flag) }
            .map { Component(glyph: $0.glyph, name: $0.name) }
    }

    /// Modifier names in display order, e.g. `["Shift", "Command"]`.
    static func names(_ flags: CGEventFlags) -> [String] {
        orderedBits.filter { flags.contains($0.flag) }.map(\.name)
    }

    private static func flags(forLegacy raw: String) -> CGEventFlags {
        switch raw {
        case "control": return .maskControl
        case "option": return .maskAlternate
        case "command": return .maskCommand
        default: return .maskShift
        }
    }
}
