import CoreGraphics
import XCTest

/// Tests drive the interceptor through `handleEvent`/`watchdogTick` with
/// synthetic CGEvents and injected seams (warp, suppression, clock, physical
/// modifier state) - the same code paths the real event tap exercises, minus
/// the tap itself.
final class GestureInterceptorTests: XCTestCase {

    // MARK: - Harness

    final class Recorder: MagnificationEmitting {
        var phases: [GesturePhase] = []
        func emit(magnification: CGFloat, phase: GesturePhase) {
            phases.append(phase)
        }
    }

    final class Harness {
        let recorder = Recorder()
        var warps = 0
        var suppressions: [CFTimeInterval] = []
        /// What the hardware (HID) reports - independent of event flags.
        var physical: CGEventFlags = []
        var clock: TimeInterval = 1000

        private(set) var interceptor: GestureInterceptor!

        init(anchor: Bool = true) {
            interceptor = GestureInterceptor(
                emitter: recorder,
                shouldIntercept: { true },
                getRequiredModifiers: { .maskShift },
                getInputSource: { .mouseDrag },
                getSensitivity: { 1.0 },
                getAnchorCursor: { anchor },
                warpCursor: { [unowned self] _ in warps += 1 },
                applySuppressionInterval: { [unowned self] in suppressions.append($0) },
                physicalModifierFlags: { [unowned self] in physical },
                now: { [unowned self] in clock }
            )
        }

        @discardableResult
        func handle(_ type: CGEventType, _ event: CGEvent) -> Unmanaged<CGEvent>? {
            interceptor.handleEvent(type: type, event: event)
        }
    }

    private func moveEvent(flags: CGEventFlags, dy: Double = 4) -> CGEvent {
        let e = CGEvent(
            mouseEventSource: nil, mouseType: .mouseMoved,
            mouseCursorPosition: CGPoint(x: 200, y: 200), mouseButton: .left
        )!
        e.flags = flags
        e.setDoubleValueField(.mouseEventDeltaY, value: dy)
        return e
    }

    private func flagsEvent(_ flags: CGEventFlags) -> CGEvent {
        let e = CGEvent(source: nil)!
        e.type = .flagsChanged
        e.flags = flags
        return e
    }

    /// Modifier down + a few moves: the standard way into an active gesture.
    private func beginGesture(_ h: Harness) {
        h.handle(.flagsChanged, flagsEvent(.maskShift))
        for _ in 0..<3 {
            h.handle(.mouseMoved, moveEvent(flags: .maskShift))
        }
    }

    // MARK: - Baseline behavior

    func testGestureBeginsAndWarpsWhileModifierHeld() {
        let h = Harness()
        h.physical = .maskShift
        beginGesture(h)

        XCTAssertTrue(h.interceptor.gestureActive)
        XCTAssertEqual(h.recorder.phases.first, .began)
        XCTAssertEqual(h.warps, 3, "anchoring should warp on every move")
        XCTAssertEqual(h.suppressions.last, 0.07, "warp suppression interval applied")

        // Moves are consumed while zooming so the cursor stays pinned.
        // (Keep the event alive across the assert: the returned Unmanaged is
        // unretained, and reflecting a dangling pointer crashes.)
        let event = moveEvent(flags: .maskShift)
        XCTAssertTrue(h.handle(.mouseMoved, event) == nil)
    }

    func testFlagsChangedReleaseEndsGesture() {
        let h = Harness()
        h.physical = .maskShift
        beginGesture(h)

        h.physical = []
        h.handle(.flagsChanged, flagsEvent([]))

        XCTAssertEqual(h.recorder.phases.last, .ended)
        XCTAssertFalse(h.interceptor.gestureActive)
        XCTAssertEqual(h.suppressions.last, 0.25, "default suppression restored")
    }

    func testCleanFlagsMoveEndsGesture() {
        let h = Harness()
        h.physical = .maskShift
        beginGesture(h)

        h.physical = []
        let event = moveEvent(flags: [])
        let passedThrough = h.handle(.mouseMoved, event) != nil

        XCTAssertEqual(h.recorder.phases.last, .ended)
        XCTAssertFalse(h.interceptor.gestureActive)
        XCTAssertTrue(passedThrough, "moves pass through once the gesture ended")
    }

    func testNoWarpWhenAnchoringDisabled() {
        let h = Harness(anchor: false)
        h.physical = .maskShift
        beginGesture(h)
        XCTAssertTrue(h.interceptor.gestureActive)
        XCTAssertEqual(h.warps, 0)
    }

    // MARK: - The incident: cursor pinned forever while the mouse keeps moving

    /// 2026-06-10 field failure. The modifier is physically released but the
    /// move events still carry the modifier flag (stale session state - e.g. a
    /// lost key-up). Movement continues, so the stillness watchdog never fires,
    /// and every move re-warps the cursor: pinned forever. The fix must end the
    /// gesture within one watchdog tick using the PHYSICAL (HID) state, and
    /// stale-flag moves must not re-pin afterwards.
    func testWedge_physicalReleaseWhileMoving_recoversWithinOneTick() {
        let h = Harness()
        h.physical = .maskShift
        beginGesture(h)
        XCTAssertTrue(h.interceptor.gestureActive)

        // Hardware releases Shift; events keep lying.
        h.physical = []
        h.handle(.mouseMoved, moveEvent(flags: .maskShift)) // movement continues
        h.interceptor.watchdogTick()

        XCTAssertEqual(h.recorder.phases.last, .ended, "watchdog must end on physical release")
        XCTAssertFalse(h.interceptor.gestureActive)

        // Stale-flag moves keep arriving: must NOT re-pin the cursor.
        let warpsAfterEnd = h.warps
        for _ in 0..<5 {
            let event = moveEvent(flags: .maskShift)
            let passedThrough = h.handle(.mouseMoved, event) != nil
            XCTAssertTrue(passedThrough, "blocked moves must pass through (cursor free)")
        }
        XCTAssertEqual(h.warps, warpsAfterEnd, "no warps after forced end")
        XCTAssertFalse(h.interceptor.gestureActive)

        // A real physical press re-arms immediately.
        h.physical = .maskShift
        h.handle(.mouseMoved, moveEvent(flags: .maskShift))
        XCTAssertTrue(h.interceptor.gestureActive, "physical re-press re-arms the gesture")
    }

    /// macOS disabled the tap (callback too slow); events - including the
    /// modifier key-up - were dropped meanwhile. Bookkeeping must reset before
    /// the tap is re-enabled so a stuck gesture can't keep pinning the cursor.
    func testTapDisabledByTimeout_resetsState() {
        let h = Harness()
        h.physical = .maskShift
        beginGesture(h)
        XCTAssertTrue(h.interceptor.gestureActive)

        h.handle(.tapDisabledByTimeout, flagsEvent(.maskShift))

        XCTAssertFalse(h.interceptor.gestureActive)
        XCTAssertFalse(h.interceptor.modifierHeld)
        XCTAssertEqual(h.suppressions.last, 0.25, "suppression restored on reset")
    }

    // MARK: - Screen sharing (session-only modifiers) must keep working

    /// Remote sessions can hold the modifier purely at the session level (the
    /// HID state stays empty). The physical check must not kill those gestures;
    /// the stillness watchdog remains their recovery path.
    func testSessionOnlyModifiers_notKilledByPhysicalCheck() {
        let h = Harness()
        h.physical = [] // hardware never sees the modifier
        beginGesture(h)
        XCTAssertTrue(h.interceptor.gestureActive)

        h.interceptor.watchdogTick()
        XCTAssertTrue(h.interceptor.gestureActive, "physical check must not fire")
        XCTAssertNotEqual(h.recorder.phases.last, .ended)

        // Stillness recovery still applies.
        h.clock += 0.3
        h.interceptor.watchdogTick()
        XCTAssertEqual(h.recorder.phases.last, .ended)
        XCTAssertFalse(h.interceptor.gestureActive)
    }

    // MARK: - Soak

    /// The wedge + recovery cycle, repeated. Catches any state that leaks from
    /// one forced end into the next gesture (cooldown, modifier bookkeeping,
    /// suppression pairing).
    func testSoak_repeatedWedgeRecoveryNeverSticks() {
        let h = Harness()
        for cycle in 0..<300 {
            h.physical = .maskShift
            h.handle(.flagsChanged, flagsEvent(.maskShift))
            h.handle(.mouseMoved, moveEvent(flags: .maskShift))
            XCTAssertTrue(h.interceptor.gestureActive, "cycle \(cycle): begin failed")

            h.physical = []
            h.handle(.mouseMoved, moveEvent(flags: .maskShift))
            h.interceptor.watchdogTick()
            XCTAssertFalse(h.interceptor.gestureActive, "cycle \(cycle): stuck after tick")

            let warps = h.warps
            for _ in 0..<3 { h.handle(.mouseMoved, moveEvent(flags: .maskShift)) }
            XCTAssertEqual(h.warps, warps, "cycle \(cycle): cursor re-pinned by stale flags")

            h.clock += 0.05
        }
        // Suppression intervals must pair: every begin (0.07) has its end (0.25),
        // and the sequence never ends pinned.
        XCTAssertEqual(h.suppressions.last, 0.25)
    }
}
