import CoreGraphics
import Foundation

/// Reads the pre-`Shortcut` settings so `ShortcutStore` can migrate them: the
/// modifier-only combo ("modifierFlags", CGEventFlags bits) and, before that,
/// the single-key "modifierKey" string.
enum ModifierStore {
    private static let key = "modifierFlags"
    private static let legacyKey = "modifierKey"

    static func legacyFlags() -> CGEventFlags {
        let defaults = UserDefaults.standard
        if defaults.object(forKey: key) != nil {
            let flags = CGEventFlags(rawValue: UInt64(bitPattern: Int64(defaults.integer(forKey: key))))
            return flags.intersection(Shortcut.modifierMask).isEmpty ? .maskShift : flags
        }
        switch defaults.string(forKey: legacyKey) {
        case "control": return .maskControl
        case "option": return .maskAlternate
        case "command": return .maskCommand
        default: return .maskShift
        }
    }
}
