import CoreGraphics

protocol MagnificationEmitting: AnyObject {
    func emit(magnification: CGFloat, phase: GesturePhase)
}

enum GesturePhase: Int64 {
    case began = 1
    case changed = 2
    case ended = 4
}
