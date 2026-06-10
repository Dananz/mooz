import CoreGraphics
import AppKit
import os

private let logger = Logger(subsystem: "com.dananz.mooz", category: "Interceptor")

// File-based diagnostic logging. OFF by default: this runs inside the
// session event-tap callback, and synchronous per-event disk I/O stalls the
// tap, which backpressures the entire input stream (frozen cursor/clicks).
// Enable only for debugging via `defaults write com.dananz.mooz diag -bool YES`.
private let diagEnabled = UserDefaults.standard.bool(forKey: "diag")
private func diagLog(_ msg: String) {
    guard diagEnabled else { return }
    let path = "/tmp/mooz-debug.log"
    let line = "\(Date()): \(msg)\n"
    if let handle = FileHandle(forWritingAtPath: path) {
        handle.seekToEndOfFile()
        handle.write(line.data(using: .utf8)!)
        handle.closeFile()
    } else {
        FileManager.default.createFile(atPath: path, contents: line.data(using: .utf8))
    }
}

protocol GestureIntercepting: AnyObject {
    func start() throws
    func stop()
    var isRunning: Bool { get }
}

enum GestureInterceptorError: Error {
    case eventTapCreationFailed
    case accessibilityNotGranted
}

final class GestureInterceptor: GestureIntercepting, @unchecked Sendable {
    /// The modifier bits we recognize. The required combo is matched exactly
    /// against these, so e.g. ⇧⌘ requires both Shift and Command and nothing
    /// else, never just Shift.
    static let trackedModifierMask: CGEventFlags = [.maskShift, .maskControl, .maskAlternate, .maskCommand]

    enum InputSource: String, CaseIterable {
        case mouseDrag, scrollWheel
    }

    private let emitter: any MagnificationEmitting
    private let shouldIntercept: () -> Bool
    private let getRequiredModifiers: () -> CGEventFlags
    private let getInputSource: () -> InputSource
    private let getSensitivity: () -> Double
    private let getAnchorCursor: () -> Bool

    // Side-effect seams. Production defaults are wired in init; tests inject
    // recorders/fakes so the full event flow runs without a real event tap.
    private let warpCursor: (CGPoint) -> Void
    private let applySuppressionInterval: (CFTimeInterval) -> Void
    private let physicalModifierFlags: () -> CGEventFlags
    private let now: () -> TimeInterval

    /// True while every configured modifier is held. Extra modifiers are
    /// allowed (subset match), so the gesture keeps firing as long as the combo
    /// is down even if another key is brushed — exact-match would drop it.
    private func modifiersMatch(_ flags: CGEventFlags) -> Bool {
        let required = getRequiredModifiers().intersection(Self.trackedModifierMask)
        guard !required.isEmpty else { return false }
        return flags.contains(required)
    }

    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private(set) var modifierHeld = false
    private(set) var gestureActive = false
    private var smoothedDeltaY: Double = 0
    private var lastGestureTime: TimeInterval = 0
    private var cursorAnchor: CGPoint = .zero
    private var watchdog: Timer?
    private(set) var isRunning = false

    /// Whether the current gesture began with the modifier physically held
    /// (HID state). Session-only gestures (screen sharing posts modifiers the
    /// hardware never sees) are exempt from the physical-release watchdog.
    private var gestureBeganWithPhysicalModifier = false
    /// Uptime of the last physical-release force-end. While recent, stale
    /// session flags alone may NOT restart a gesture - that's the loop that
    /// pinned the cursor forever (2026-06-10 incident).
    private var physicalForceEndUptime: TimeInterval = -.infinity
    private static let physicalForceEndCooldown: TimeInterval = 2.0

    /// Default local-events suppression interval (0.25s) vs the value that lets
    /// repeated CGWarpMouseCursorPosition calls pin the cursor without the
    /// quarter-second post-warp freeze. Values are Mac Mouse Fix's.
    private static let warpSuppressionInterval: CFTimeInterval = 0.07
    private static let defaultSuppressionInterval: CFTimeInterval = 0.25

    // Suppression is applied through the injected seam (see init).

    /// Magnification mapping. Per-pixel rate is multiplied by the user
    /// sensitivity (0.1–3.0); each event's delta is clamped so a single large
    /// movement can't jump the zoom.
    private static let magnificationPerPixel = 0.01
    private static let maxMagnificationPerEvent = 0.15
    private static func clampMagnification(_ value: Double) -> Double {
        max(-maxMagnificationPerEvent, min(maxMagnificationPerEvent, value))
    }

    /// Starts/ends a zoom gesture and freezes the cursor in lockstep. The cursor
    /// is pinned by warping it back to its anchor every frame (see the drag
    /// handler). For that to work smoothly we must shrink the local-events
    /// suppression interval at gesture start (otherwise each warp triggers a
    /// ~0.25s pointer freeze = stutter) and restore it on end. This is exactly
    /// what Mac Mouse Fix ships; they found CGAssociateMouseAndMouseCursorPosition
    /// makes the input deltas "inaccurate and erratic", so we don't use it.
    ///
    /// Warp-based freeze has no global sticky state to strand: the cursor is only
    /// held while moves are actively arriving. The watchdog still force-ends a
    /// stalled gesture so the suppression interval is always restored.
    private func setGestureActive(_ active: Bool) {
        guard active != gestureActive else { return }
        gestureActive = active
        if active {
            smoothedDeltaY = 0
            lastGestureTime = now()
            if getAnchorCursor() {
                applySuppressionInterval(Self.warpSuppressionInterval)
            }
            diagLog("Gesture active (anchor=\(getAnchorCursor()))")
            startWatchdog()
        } else {
            applySuppressionInterval(Self.defaultSuppressionInterval)
            stopWatchdog()
            smoothedDeltaY = 0
        }
    }

    private func startWatchdog() {
        watchdog?.invalidate()
        watchdog = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] timer in
            guard let self else { timer.invalidate(); return }
            self.watchdogTick()
        }
    }

    /// One watchdog pass. Internal so tests drive it directly, without timers.
    func watchdogTick() {
        guard gestureActive else { return }
        // The hardware (HID) modifier state is authoritative: combo no longer
        // physically held → end NOW, even while moves keep arriving. The old
        // stillness-only check never fired while the mouse was moving, which
        // left the cursor permanently pinned when session state got stuck.
        if gestureBeganWithPhysicalModifier, !modifiersMatch(physicalModifierFlags()) {
            diagLog("WATCHDOG force-end (modifier physically released)")
            physicalForceEndUptime = now()
            modifierHeld = false
            emitter.emit(magnification: 0, phase: .ended)
            setGestureActive(false)
            return
        }
        // No movement for a moment → a missed modifier-release or a window
        // stealing focus mid-gesture. End cleanly so the cursor unfreezes.
        if now() - lastGestureTime > 0.2 {
            diagLog("WATCHDOG force-end (no movement 0.2s)")
            emitter.emit(magnification: 0, phase: .ended)
            setGestureActive(false)
        }
    }

    private func stopWatchdog() {
        watchdog?.invalidate()
        watchdog = nil
    }

    /// Starts the gesture and anchors the cursor at `location` on the first
    /// event while the modifier is held. Shared by drag and scroll paths.
    private func beginGestureIfNeeded(at location: CGPoint) {
        guard !gestureActive else { return }
        let physicallyHeld = modifiersMatch(physicalModifierFlags())
        // After a physical-release force-end, event flags saying "still held"
        // are exactly the lie that wedged the cursor. Demand physical truth to
        // re-arm until the cooldown passes.
        if !physicallyHeld, now() - physicalForceEndUptime < Self.physicalForceEndCooldown {
            return
        }
        gestureBeganWithPhysicalModifier = physicallyHeld
        cursorAnchor = location
        emitter.emit(magnification: 0, phase: .began)
        setGestureActive(true)
    }

    init(
        emitter: any MagnificationEmitting,
        shouldIntercept: @escaping () -> Bool,
        getRequiredModifiers: @escaping () -> CGEventFlags,
        getInputSource: @escaping () -> InputSource,
        getSensitivity: @escaping () -> Double,
        getAnchorCursor: @escaping () -> Bool,
        warpCursor: @escaping (CGPoint) -> Void = { CGWarpMouseCursorPosition($0) },
        applySuppressionInterval: @escaping (CFTimeInterval) -> Void = { interval in
            guard let src = CGEventSource(stateID: .combinedSessionState) else { return }
            src.localEventsSuppressionInterval = interval
        },
        // HID hardware modifier state: authoritative even when session-level
        // bookkeeping (event flags) is stale or our tap dropped the key-up.
        physicalModifierFlags: @escaping () -> CGEventFlags = {
            CGEventSource.flagsState(.hidSystemState)
        },
        now: @escaping () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    ) {
        self.emitter = emitter
        self.shouldIntercept = shouldIntercept
        self.getRequiredModifiers = getRequiredModifiers
        self.getInputSource = getInputSource
        self.getSensitivity = getSensitivity
        self.getAnchorCursor = getAnchorCursor
        self.warpCursor = warpCursor
        self.applySuppressionInterval = applySuppressionInterval
        self.physicalModifierFlags = physicalModifierFlags
        self.now = now
    }

    func start() throws {
        guard AXIsProcessTrusted() else {
            throw GestureInterceptorError.accessibilityNotGranted
        }
        guard eventTap == nil else { return }

        // Clear state a prior run may have stranded if it was killed
        // mid-gesture: re-associate the cursor and restore the suppression
        // interval. Both are harmless if already in their default state.
        CGAssociateMouseAndMouseCursorPosition(1)
        applySuppressionInterval(Self.defaultSuppressionInterval)

        // mouseMoved is tapped so zoom triggers on modifier+move (no button),
        // the Logitech-style gesture. While zooming, the move is consumed so
        // the cursor/page stays anchored. Safe because the per-event disk I/O
        // that previously stalled this callback is now gated off (diagLog).
        let eventMask: CGEventMask = (
            (1 << CGEventType.mouseMoved.rawValue) |
            (1 << CGEventType.leftMouseDragged.rawValue) |
            (1 << CGEventType.rightMouseDragged.rawValue) |
            (1 << CGEventType.scrollWheel.rawValue) |
            (1 << CGEventType.flagsChanged.rawValue)
        )

        let userInfo = Unmanaged.passUnretained(self).toOpaque()

        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: eventMask,
            callback: GestureInterceptor.tapCallback,
            userInfo: userInfo
        ) else {
            throw GestureInterceptorError.eventTapCreationFailed
        }

        eventTap = tap
        runLoopSource = CFMachPortCreateRunLoopSource(nil, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetCurrent(), runLoopSource, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        isRunning = true
        let tapEnabled = CGEvent.tapIsEnabled(tap: tap)
        diagLog("Event tap started successfully. tapEnabled=\(tapEnabled) runLoop=\(CFRunLoopGetCurrent()!) mainRunLoop=\(CFRunLoopGetMain()!)")

        // Heartbeat: check tap state every 5 seconds
        Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { [weak self] timer in
            guard let self = self else { timer.invalidate(); return }
            if let tap = self.eventTap {
                let enabled = CGEvent.tapIsEnabled(tap: tap)
                diagLog("Heartbeat: tapEnabled=\(enabled) eventCount=\(self.eventCount)")
                if !enabled {
                    diagLog("⚠️ Tap was disabled! Re-enabling...")
                    // Same reset as tapDisabledByTimeout: events were lost.
                    self.modifierHeld = false
                    self.setGestureActive(false)
                    CGEvent.tapEnable(tap: tap, enable: true)
                }
            } else {
                diagLog("Heartbeat: eventTap is nil!")
                timer.invalidate()
            }
        }
    }

    func stop() {
        setGestureActive(false) // never leave the cursor locked
        if let tap = eventTap {
            CGEvent.tapEnable(tap: tap, enable: false)
            if let source = runLoopSource {
                CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, .commonModes)
            }
        }
        eventTap = nil
        runLoopSource = nil
        isRunning = false
    }

    private static let tapCallback: CGEventTapCallBack = { _, type, event, userInfo in
        guard let userInfo = userInfo else { return Unmanaged.passUnretained(event) }
        let interceptor = Unmanaged<GestureInterceptor>.fromOpaque(userInfo).takeUnretainedValue()
        return interceptor.handleEvent(type: type, event: event)
    }

    private var eventCount = 0
    /// Internal so tests can feed synthetic CGEvents through the real flow.
    func handleEvent(type: CGEventType, event: CGEvent) -> Unmanaged<CGEvent>? {
        let passthrough = Unmanaged.passUnretained(event)

        // Only log non-mouseMoved events to avoid tap timeout from heavy I/O
        eventCount += 1
        if type != .mouseMoved {
            diagLog("handleEvent #\(eventCount): type=\(type.rawValue)")
        }

        // Re-enable tap only on timeout (callback took too long).
        // Do NOT re-enable on tapDisabledByUserInput — that means the user
        // or system intentionally disabled it (e.g., revoking Accessibility permission).
        if type == .tapDisabledByTimeout {
            diagLog("⚠️ TAP DISABLED BY TIMEOUT — re-enabling")
            // Events (including the modifier key-up) were dropped while the tap
            // was off: the bookkeeping can't be trusted. Reset before
            // re-enabling so a stuck gesture can't keep pinning the cursor.
            modifierHeld = false
            setGestureActive(false)
            if let tap = eventTap {
                CGEvent.tapEnable(tap: tap, enable: true)
            }
            return passthrough
        }

        if type == .tapDisabledByUserInput {
            diagLog("⚠️ TAP DISABLED BY USER INPUT — tearing down")
            // Permission revoked — fully tear down the tap and reset all state
            modifierHeld = false
            setGestureActive(false) // reconnect cursor if it was locked
            if let tap = eventTap {
                CGEvent.tapEnable(tap: tap, enable: false)
                if let source = runLoopSource {
                    CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, .commonModes)
                }
            }
            eventTap = nil
            runLoopSource = nil
            isRunning = false
            return passthrough
        }

        // Track modifier key state
        if type == .flagsChanged {
            let flags = event.flags
            let wasHeld = modifierHeld
            modifierHeld = modifiersMatch(flags)
            diagLog("flagsChanged: rawFlags=\(flags.rawValue) required=\(getRequiredModifiers().rawValue) modifierHeld=\(modifierHeld) wasHeld=\(wasHeld)")

            if modifierHeld != wasHeld {
                diagLog("Modifier \(self.modifierHeld ? "PRESSED" : "RELEASED")")
            }

            // End gesture when modifier is released
            if wasHeld && !modifierHeld && gestureActive {
                emitter.emit(magnification: 0, phase: .ended)
                setGestureActive(false)
            }
            return passthrough
        }

        // Check modifier from event flags directly (screen sharing may not send flagsChanged)
        if type == .scrollWheel || type == .mouseMoved || type == .leftMouseDragged || type == .rightMouseDragged {
            let nowHeld = modifiersMatch(event.flags)
            if nowHeld != modifierHeld {
                diagLog("Modifier state updated from event flags: \(nowHeld) (was \(modifierHeld)) rawFlags=\(event.flags.rawValue)")
                modifierHeld = nowHeld
            }
        }

        // If modifier not held or interception disabled, end any active gesture and pass through
        guard modifierHeld, shouldIntercept() else {
            if gestureActive {
                emitter.emit(magnification: 0, phase: .ended)
                setGestureActive(false)
            }
            return passthrough
        }

        let inputSource = getInputSource()
        let sensitivity = getSensitivity()

        switch type {
        case .mouseMoved, .leftMouseDragged, .rightMouseDragged:
            let anchorCursor = getAnchorCursor()
            // In scroll mode a mouse move's only job is to keep the cursor
            // anchored. If anchoring is off there's nothing to do — let it move.
            if inputSource != .mouseDrag && !anchorCursor {
                return passthrough
            }
            // Anchor the cursor whenever the modifier is held, in BOTH modes.
            // (In scroll mode the move only pins the pointer; the wheel zooms.)
            beginGestureIfNeeded(at: event.location)
            // Begin can refuse (post-force-end cooldown): let the move flow.
            guard gestureActive else { return passthrough }
            if anchorCursor { warpCursor(cursorAnchor) }
            lastGestureTime = now()

            if inputSource == .mouseDrag {
                // Low-pass the raw pixel delta (the biggest "buttery" factor),
                // map to a magnification delta, clamp per-event to kill spikes.
                let rawDeltaY = event.getDoubleValueField(.mouseEventDeltaY)
                smoothedDeltaY = 0.35 * rawDeltaY + 0.65 * smoothedDeltaY
                let magnification = Self.clampMagnification(-smoothedDeltaY * sensitivity * Self.magnificationPerPixel)
                if abs(magnification) > 0.0002 {
                    emitter.emit(magnification: magnification, phase: .changed)
                }
                diagLog("DRAG zoom: raw=\(rawDeltaY) smooth=\(smoothedDeltaY) mag=\(magnification)")
            }
            return nil // consume so the cursor stays frozen in both modes

        case .scrollWheel:
            guard inputSource == .scrollWheel else { return passthrough }
            // Pixel delta (pointDelta); check BOTH axes since macOS turns
            // modifier+scroll into a horizontal scroll (delta moves to axis 2).
            let deltaY = event.getDoubleValueField(.scrollWheelEventPointDeltaAxis1)
            let deltaX = event.getDoubleValueField(.scrollWheelEventPointDeltaAxis2)
            let deltaPixel = abs(deltaY) > abs(deltaX) ? deltaY : deltaX
            let magnification = Self.clampMagnification(deltaPixel * sensitivity * Self.magnificationPerPixel)

            beginGestureIfNeeded(at: event.location)
            guard gestureActive else { return passthrough }
            if getAnchorCursor() { warpCursor(cursorAnchor) }
            lastGestureTime = now()
            if abs(magnification) > 0.0005 {
                emitter.emit(magnification: magnification, phase: .changed)
            }
            diagLog("Scroll zoom: used=\(deltaPixel) mag=\(magnification)")
            return nil

        default:
            return passthrough
        }
    }
}
