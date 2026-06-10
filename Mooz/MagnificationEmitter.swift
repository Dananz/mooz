import CoreGraphics

// MagnificationEmitting + GesturePhase live in MagnificationEmitting.swift so the
// unit-test target can compile the interceptor without this class's C dependency.

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
