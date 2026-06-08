import CoreGraphics

protocol MagnificationEmitting: AnyObject {
    func emit(magnification: CGFloat, phase: GesturePhase)
}

enum GesturePhase: Int64 {
    case began = 1
    case changed = 2
    case ended = 4
}

/// Synthesizes a macOS magnify (pinch-to-zoom) gesture as a full IOHID gesture
/// blob via `MoozEmitMagnify` (original implementation in `MoozGesture.m`).
///
/// We previously set four loose CGEvent fields (type 29, 110/132/113). That
/// zooms Chrome/Safari but NOT Gecko browsers (Firefox/Zen): Gecko only acts on
/// a properly-structured gesture event. The full serialized blob is what a real
/// trackpad sends, so every browser accepts it.
final class MagnificationEmitter: MagnificationEmitting {
    func emit(magnification: CGFloat, phase: GesturePhase) {
        // phase.rawValue is the IOHID phase bit (1 began / 2 changed / 4 ended).
        MoozEmitMagnify(Double(magnification), Int32(phase.rawValue))
    }
}
