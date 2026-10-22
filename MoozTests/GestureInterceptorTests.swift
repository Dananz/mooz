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
        var physicalInputs: Set<ShortcutInput> = []
        var clock: TimeInterval = 1000
        var posted: [CGEvent] = []

        private(set) var interceptor: GestureInterceptor!

        init(
            anchor: Bool = true,
            shortcut: Shortcut = .default,
            source: GestureInterceptor.InputSource = .mouseDrag
        ) {
            interceptor = GestureInterceptor(
                emitter: recorder,
                shouldIntercept: { true },
                getShortcut: { shortcut },
                getInputSource: { source },
                getSensitivity: { 1.0 },
                getAnchorCursor: { anchor },
                warpCursor: { [unowned self] _ in warps += 1 },
                applySuppressionInterval: { [unowned self] in suppressions.append($0) },
                physicalModifierFlags: { [unowned self] in physical },
                isInputPhysicallyDown: { [unowned self] in physicalInputs.contains($0) },
                postEvent: { [unowned self] in posted.append($0) },
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

    private func keyEvent(_ code: UInt16, down: Bool, flags: CGEventFlags = [], repeating: Bool = false) -> CGEvent {
        let e = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down)!
        e.flags = flags
        if repeating { e.setIntegerValueField(.keyboardEventAutorepeat, value: 1) }
        return e
    }

    private func buttonEvent(_ button: Int, down: Bool, flags: CGEventFlags = []) -> CGEvent {
        let e = CGEvent(
            mouseEventSource: nil, mouseType: down ? .otherMouseDown : .otherMouseUp,
            mouseCursorPosition: CGPoint(x: 200, y: 200), mouseButton: .center
        )!
        e.setIntegerValueField(.mouseEventButtonNumber, value: Int64(button))
        e.flags = flags
        return e
    }

    private func scrollEvent(flags: CGEventFlags = [], dy: Int32 = 5) -> CGEvent {
        let e = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: dy, wheel2: 0, wheel3: 0)!
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

    // MARK: - Keys and mouse buttons in the shortcut

    private let zKey: UInt16 = 6 // kVK_ANSI_Z
    private let aKey: UInt16 = 0 // kVK_ANSI_A

    func testShortcutModelMatchesAndRoundTrips() {
        let s = Shortcut(modifiers: [.maskControl, .maskShift], inputs: [.key(6), .button(2)])
        XCTAssertEqual(Shortcut(rawValue: s.rawValue), s)
        XCTAssertNil(Shortcut(rawValue: "x1"))
        XCTAssertFalse(s.isSatisfied(flags: [.maskControl, .maskShift], held: [.key(6)]))
        XCTAssertTrue(s.isSatisfied(flags: [.maskControl, .maskShift, .maskCommand], held: [.key(6), .button(2), .key(0)]))
        XCTAssertFalse(Shortcut.empty.isSatisfied(flags: .maskShift, held: []))
    }

    func testMultipleModifiersRequireEveryOne() {
        let h = Harness(shortcut: Shortcut(modifiers: [.maskControl, .maskShift]))
        h.physical = [.maskControl, .maskShift]
        h.handle(.flagsChanged, flagsEvent(.maskShift))
        h.handle(.mouseMoved, moveEvent(flags: .maskShift))
        XCTAssertFalse(h.interceptor.gestureActive, "Shift alone is not ⌃⇧")

        h.handle(.flagsChanged, flagsEvent([.maskControl, .maskShift]))
        h.handle(.mouseMoved, moveEvent(flags: [.maskControl, .maskShift]))
        XCTAssertTrue(h.interceptor.gestureActive)
    }

    func testKeyShortcutZoomsAndSwallowsTheKey() {
        let h = Harness(shortcut: Shortcut(modifiers: [], inputs: [.key(zKey)]))
        h.physicalInputs = [.key(zKey)]
        let down = keyEvent(zKey, down: true)
        XCTAssertTrue(h.handle(.keyDown, down) == nil, "press held back")
        let repeated = keyEvent(zKey, down: true, repeating: true)
        XCTAssertTrue(h.handle(.keyDown, repeated) == nil, "repeat held back")

        h.handle(.mouseMoved, moveEvent(flags: []))
        h.handle(.mouseMoved, moveEvent(flags: []))
        XCTAssertTrue(h.interceptor.gestureActive)
        XCTAssertTrue(h.recorder.phases.contains(.changed))

        h.physicalInputs = []
        let up = keyEvent(zKey, down: false)
        XCTAssertTrue(h.handle(.keyUp, up) == nil, "release of a zoom press is swallowed")
        XCTAssertFalse(h.interceptor.gestureActive)
        XCTAssertEqual(h.recorder.phases.last, .ended)
        XCTAssertTrue(h.posted.isEmpty, "nothing replayed after a zoom")
    }

    func testKeyPressWithoutZoomIsReplayedOnRelease() {
        let h = Harness(shortcut: Shortcut(modifiers: [], inputs: [.key(zKey)]))
        let down = keyEvent(zKey, down: true)
        XCTAssertTrue(h.handle(.keyDown, down) == nil)
        let up = keyEvent(zKey, down: false)
        XCTAssertTrue(h.handle(.keyUp, up) == nil)

        XCTAssertEqual(h.posted.map { $0.type }, [.keyDown, .keyUp])
        XCTAssertTrue(h.posted.allSatisfy { $0.getIntegerValueField(.eventSourceUserData) == GestureInterceptor.replayMarker })

        // Replays come back through the tap and must pass untouched.
        let replay = h.posted[0]
        XCTAssertTrue(h.handle(.keyDown, replay) != nil)
    }

    func testTypingAnotherKeyReleasesHeldBackPressInOrder() {
        let h = Harness(shortcut: Shortcut(modifiers: [], inputs: [.key(zKey)]))
        let z = keyEvent(zKey, down: true)
        h.handle(.keyDown, z)
        let a = keyEvent(aKey, down: true)
        XCTAssertTrue(h.handle(.keyDown, a) == nil)
        XCTAssertEqual(h.posted.map { $0.getIntegerValueField(.keyboardEventKeycode) }, [Int64(zKey), Int64(aKey)])

        // Z already reached the app, so its release passes through.
        let zUp = keyEvent(zKey, down: false)
        XCTAssertTrue(h.handle(.keyUp, zUp) != nil)
        XCTAssertEqual(h.posted.count, 2)
    }

    func testKeyPassesThroughWhenModifiersOfTheShortcutAreNotHeld() {
        let h = Harness(shortcut: Shortcut(modifiers: .maskControl, inputs: [.key(zKey)]))
        let plain = keyEvent(zKey, down: true)
        XCTAssertTrue(h.handle(.keyDown, plain) != nil, "typing z alone is untouched")
        let up = keyEvent(zKey, down: false)
        XCTAssertTrue(h.handle(.keyUp, up) != nil)
        XCTAssertTrue(h.posted.isEmpty)
    }

    func testMouseButtonShortcutWithScroll() {
        let shortcut = Shortcut(modifiers: .maskShift, inputs: [.button(3)])
        let h = Harness(shortcut: shortcut, source: .scrollWheel)
        h.physical = .maskShift
        h.physicalInputs = [.button(3)]

        let scrollShiftOnly = scrollEvent(flags: .maskShift)
        XCTAssertTrue(h.handle(.scrollWheel, scrollShiftOnly) != nil, "Shift alone doesn't zoom")

        let down = buttonEvent(3, down: true, flags: .maskShift)
        XCTAssertTrue(h.handle(.otherMouseDown, down) == nil, "Back held back")
        h.handle(.scrollWheel, scrollEvent(flags: .maskShift))
        XCTAssertTrue(h.interceptor.gestureActive)

        // Releasing the button physically ends the gesture via the watchdog too.
        h.physicalInputs = []
        h.interceptor.watchdogTick()
        XCTAssertFalse(h.interceptor.gestureActive)

        let up = buttonEvent(3, down: false, flags: .maskShift)
        XCTAssertTrue(h.handle(.otherMouseUp, up) == nil, "Back never navigates after a zoom")
        XCTAssertTrue(h.posted.isEmpty)
    }

    func testMouseButtonClickWithoutZoomStillClicks() {
        let h = Harness(shortcut: Shortcut(modifiers: [], inputs: [.button(2)]), source: .scrollWheel)
        h.handle(.otherMouseDown, buttonEvent(2, down: true))
        h.handle(.otherMouseUp, buttonEvent(2, down: false))
        XCTAssertEqual(h.posted.map { $0.type }, [.otherMouseDown, .otherMouseUp])
        XCTAssertEqual(h.posted.first?.getIntegerValueField(.mouseEventButtonNumber), 2)
    }

    /// The key-shortcut version of the wedge: the tap misses Z's release. The
    /// watchdog must end the gesture and forget Z, so moves after the
    /// cooldown don't re-pin the cursor.
    func testWedge_lostKeyRelease_doesNotRearmAfterCooldown() {
        let h = Harness(shortcut: Shortcut(modifiers: [], inputs: [.key(zKey)]))
        h.physicalInputs = [.key(zKey)]
        h.handle(.keyDown, keyEvent(zKey, down: true))
        h.handle(.mouseMoved, moveEvent(flags: []))
        XCTAssertTrue(h.interceptor.gestureActive)

        h.physicalInputs = [] // released, but the key-up never reaches the tap
        h.interceptor.watchdogTick()
        XCTAssertFalse(h.interceptor.gestureActive)

        h.clock += 3
        for _ in 0..<5 {
            let event = moveEvent(flags: [])
            XCTAssertTrue(h.handle(.mouseMoved, event) != nil, "moves pass through")
        }
        XCTAssertFalse(h.interceptor.gestureActive, "stale key must not re-arm the gesture")

        // A real press works again.
        h.physicalInputs = [.key(zKey)]
        h.handle(.keyDown, keyEvent(zKey, down: true))
        h.handle(.mouseMoved, moveEvent(flags: []))
        XCTAssertTrue(h.interceptor.gestureActive)
    }

    func testFreshPressAfterLostReleaseIsNotSwallowed() {
        let h = Harness(shortcut: Shortcut(modifiers: [], inputs: [.key(zKey)]))
        h.physicalInputs = [.key(zKey)]
        h.handle(.keyDown, keyEvent(zKey, down: true))
        h.handle(.mouseMoved, moveEvent(flags: []))
        h.handle(.mouseMoved, moveEvent(flags: []))
        XCTAssertTrue(h.recorder.phases.contains(.changed), "zoomed, so Z is consumed")

        // Z's release is lost; the next tap of Z must still type.
        h.handle(.keyDown, keyEvent(zKey, down: true))
        h.handle(.keyUp, keyEvent(zKey, down: false))
        XCTAssertEqual(h.posted.map { $0.type }, [.keyDown, .keyUp])
    }

    // MARK: - Review fixes

    func testLostKeyReleaseWhileIdleNeverStartsAGesture() {
        let h = Harness(shortcut: Shortcut(modifiers: [], inputs: [.key(zKey)]))
        h.physicalInputs = [.key(zKey)]
        h.handle(.keyDown, keyEvent(zKey, down: true))
        h.physicalInputs = [] // released; the key-up never reached the tap

        for _ in 0..<5 {
            let event = moveEvent(flags: [])
            XCTAssertTrue(h.handle(.mouseMoved, event) != nil, "moves pass through")
        }
        XCTAssertFalse(h.interceptor.gestureActive, "a stale key must not pin the cursor")
    }

    func testSmallDragStillCountsAsZoom() {
        let h = Harness(shortcut: Shortcut(modifiers: [], inputs: [.button(3)]))
        h.physicalInputs = [.button(3)]
        h.handle(.otherMouseDown, buttonEvent(3, down: true))
        for _ in 0..<4 { h.handle(.mouseMoved, moveEvent(flags: [], dy: 1)) }
        XCTAssertTrue(h.interceptor.gestureActive)

        h.physicalInputs = []
        let up = buttonEvent(3, down: false)
        XCTAssertTrue(h.handle(.otherMouseUp, up) == nil)
        XCTAssertTrue(h.posted.isEmpty, "Back must not fire after a drag zoom")
    }

    func testTapTimeoutHandsHeldBackPressToTheApp() {
        let h = Harness(shortcut: Shortcut(modifiers: [], inputs: [.key(zKey)]))
        h.handle(.keyDown, keyEvent(zKey, down: true))
        h.handle(.tapDisabledByTimeout, flagsEvent([]))
        XCTAssertEqual(h.posted.map { $0.type }, [.keyDown])

        let up = keyEvent(zKey, down: false)
        XCTAssertTrue(h.handle(.keyUp, up) != nil, "the release follows the replayed press")
    }

    func testEventMaskListensForKeysAndButtonsOnlyWhenUsed() {
        let keyDown: CGEventMask = 1 << CGEventType.keyDown.rawValue
        let buttonDown: CGEventMask = 1 << CGEventType.otherMouseDown.rawValue
        let modifiersOnly = GestureInterceptor.eventMask(for: .default)
        XCTAssertEqual(modifiersOnly & keyDown, 0)
        XCTAssertEqual(modifiersOnly & buttonDown, 0)
        let withKey = GestureInterceptor.eventMask(for: Shortcut(modifiers: .maskControl, inputs: [.key(zKey)]))
        XCTAssertNotEqual(withKey & keyDown, 0)
        XCTAssertEqual(withKey & buttonDown, 0)
        let withButton = GestureInterceptor.eventMask(for: Shortcut(modifiers: [], inputs: [.button(3)]))
        XCTAssertNotEqual(withButton & buttonDown, 0)
    }

    func testBareTypingKeyIsRefused() {
        XCTAssertTrue(Shortcut(modifiers: [], inputs: [.key(zKey)]).isBareTypingKey)
        XCTAssertFalse(Shortcut(modifiers: .maskControl, inputs: [.key(zKey)]).isBareTypingKey)
        XCTAssertFalse(Shortcut(modifiers: [], inputs: [.key(zKey), .button(3)]).isBareTypingKey)
        XCTAssertFalse(Shortcut(modifiers: [], inputs: [.key(0x69)]).isBareTypingKey, "F13 alone is fine")
        XCTAssertFalse(Shortcut(modifiers: [], inputs: [.button(2)]).isBareTypingKey)
    }
}
